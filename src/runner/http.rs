// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2023 Stacks Open Internet Foundation
// Copyright (C) 2023 Jude Nelson
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::collections::HashMap;
use std::io;
use std::io::{Read, Write};
use std::net::SocketAddr;

use stacks_common::codec::MAX_MESSAGE_LEN;
use stacks_common::deps_common::httparse;
use stacks_common::util::chunked_encoding::*;

use crate::runner::Error;

pub const MAX_HTTP_HEADERS: usize = 32;
pub const MAX_HTTP_HEADER_LEN: usize = 4096;

/// Decoding of the relevant parts of a signer-directed HTTP request from the Stacks node
#[derive(Debug)]
pub struct WrbHttpRequest {
    pub verb: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body_offset: usize,
}

impl WrbHttpRequest {
    pub fn new(
        verb: String,
        path: String,
        headers: HashMap<String, String>,
        body_offset: usize,
    ) -> WrbHttpRequest {
        WrbHttpRequest {
            verb,
            path,
            headers,
            body_offset,
        }
    }

    /// Decompose into (verb, path, headers, body-offset)
    pub fn destruct(self) -> (String, String, HashMap<String, String>, usize) {
        (self.verb, self.path, self.headers, self.body_offset)
    }
}

/// Decode the HTTP request payload into its headers and body.
/// Returns (verb, path, table of headers, body_offset) on success
pub fn decode_http_request(payload: &[u8]) -> Result<WrbHttpRequest, Error> {
    // realistically, there won't be more than 32 headers
    let mut headers_buf = [httparse::EMPTY_HEADER; MAX_HTTP_HEADERS];
    let mut req = httparse::Request::new(&mut headers_buf);
    let (verb, path, headers, body_offset) =
        if let Ok(httparse::Status::Complete(body_offset)) = req.parse(payload) {
            // version must be valid
            match req
                .version
                .ok_or(Error::MalformedRequest("No HTTP version".to_string()))?
            {
                0 => {}
                1 => {}
                _ => {
                    return Err(Error::MalformedRequest("Invalid HTTP version".to_string()));
                }
            };

            let verb = req
                .method
                .ok_or(Error::MalformedRequest("No HTTP method".to_string()))?
                .to_string();
            let path = req
                .path
                .ok_or(Error::MalformedRequest("No HTTP path".to_string()))?
                .to_string();

            let mut headers: HashMap<String, String> = HashMap::new();
            for i in 0..req.headers.len() {
                let value = String::from_utf8(req.headers[i].value.to_vec()).map_err(|_e| {
                    Error::MalformedRequest("Invalid HTTP header value: not utf-8".to_string())
                })?;
                if !value.is_ascii() {
                    return Err(Error::MalformedRequest(
                        "Invalid HTTP request: header value is not ASCII-US".to_string(),
                    ));
                }
                if value.len() > MAX_HTTP_HEADER_LEN {
                    return Err(Error::MalformedRequest(
                        "Invalid HTTP request: header value is too big".to_string(),
                    ));
                }

                let key = req.headers[i].name.to_string().to_lowercase();
                if headers.get(&key).is_some() {
                    return Err(Error::MalformedRequest(format!(
                        "Invalid HTTP request: duplicate header \"{}\"",
                        key
                    )));
                }
                headers.insert(key, value);
            }
            (verb, path, headers, body_offset)
        } else {
            return Err(Error::Deserialize(
                "Failed to decode HTTP headers".to_string(),
            ));
        };

    Ok(WrbHttpRequest::new(verb, path, headers, body_offset))
}

/// Decode the HTTP response payload into its headers and body.
/// Return the offset into payload where the body starts, and a table of headers.
///
/// If the payload contains a status code other than 200, then RPCERror::HttpError(..) will be
/// returned with the status code.
/// If the payload is missing necessary data, then Error::MalformedResponse(..) will be
/// returned, with a human-readable reason string.
/// If the payload does not contain a full HTTP header list, then Error::Deserialize(..) will be
/// returned.  This can happen if there are more than MAX_HTTP_HEADERS in the payload, for example.
pub fn decode_http_response(payload: &[u8]) -> Result<(HashMap<String, String>, usize), Error> {
    // realistically, there won't be more than 32 headers
    let mut headers_buf = [httparse::EMPTY_HEADER; MAX_HTTP_HEADERS];
    let mut resp = httparse::Response::new(&mut headers_buf);

    // consume respuest
    let (headers, body_offset) =
        if let Ok(httparse::Status::Complete(body_offset)) = resp.parse(payload) {
            let code = if let Some(code) = resp.code {
                code
            } else {
                return Err(Error::MalformedResponse(
                    "No HTTP status code returned".to_string(),
                ));
            };
            if let Some(version) = resp.version {
                if version != 0 && version != 1 {
                    return Err(Error::MalformedResponse(format!(
                        "Unrecognized HTTP code {}",
                        version
                    )));
                }
            } else {
                return Err(Error::MalformedResponse(
                    "No HTTP version given".to_string(),
                ));
            }
            let mut headers: HashMap<String, String> = HashMap::new();
            for i in 0..resp.headers.len() {
                let value = String::from_utf8(resp.headers[i].value.to_vec()).map_err(|_e| {
                    Error::MalformedResponse("Invalid HTTP header value: not utf-8".to_string())
                })?;
                if !value.is_ascii() {
                    return Err(Error::MalformedResponse(
                        "Invalid HTTP response: header value is not ASCII-US".to_string(),
                    ));
                }
                if value.len() > MAX_HTTP_HEADER_LEN {
                    return Err(Error::MalformedResponse(
                        "Invalid HTTP response: header value is too big".to_string(),
                    ));
                }

                let key = resp.headers[i].name.to_string().to_lowercase();
                if headers.contains_key(&key) {
                    return Err(Error::MalformedResponse(format!(
                        "Invalid HTTP respuest: duplicate header \"{}\"",
                        key
                    )));
                }
                headers.insert(key, value);
            }
            if code != 200 {
                return Err(Error::HttpError(code.into(), headers, body_offset));
            }
            (headers, body_offset)
        } else {
            return Err(Error::Deserialize(
                "Failed to decode HTTP headers".to_string(),
            ));
        };

    Ok((headers, body_offset))
}

/// Decode an HTTP body, given the headers.
pub fn decode_http_body(headers: &HashMap<String, String>, mut buf: &[u8]) -> io::Result<Vec<u8>> {
    let chunked = if let Some(val) = headers.get("transfer-encoding") {
        val == "chunked"
    } else {
        false
    };

    let body = if chunked {
        // chunked encoding
        let ptr = &mut buf;
        let mut fd = HttpChunkedTransferReader::from_reader(ptr, MAX_MESSAGE_LEN.into());
        let mut decoded_body = vec![];
        fd.read_to_end(&mut decoded_body)?;
        decoded_body
    } else {
        // body is just as-is
        buf.to_vec()
    };

    Ok(body)
}

/// Run an HTTP request, synchronously, through the given read/write handle
/// Return the HTTP reply, decoded if it was chunked
pub fn run_http_request<S: Read + Write>(
    sock: &mut S,
    host: &SocketAddr,
    verb: &str,
    path: &str,
    content_type: Option<&str>,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let content_length_hdr = if !payload.is_empty() {
        format!("Content-Length: {}\r\n", payload.len())
    } else {
        "".to_string()
    };

    let req_txt = if let Some(content_type) = content_type {
        format!(
            "{} {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\nContent-Type: {}\r\n{}User-Agent: wrb/0.1\r\nAccept: */*\r\n\r\n",
            verb, path, host, content_type, content_length_hdr
        )
    } else {
        format!(
            "{} {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n{}User-Agent: wrb/0.1\r\nAccept: */*\r\n\r\n",
            verb, path, host, content_length_hdr
        )
    };
    wrb_debug!("HTTP request\n{}", &req_txt);

    sock.write_all(req_txt.as_bytes())?;
    sock.write_all(payload)?;

    let mut buf = vec![];

    sock.read_to_end(&mut buf)?;

    let (code, headers, body_offset) = match decode_http_response(&buf) {
        Ok((headers, body_offset)) => (200, headers, body_offset),
        Err(Error::HttpError(code, headers, body_offset)) => (code, headers, body_offset),
        Err(e) => {
            return Err(e);
        }
    };

    if body_offset >= buf.len() {
        // no body
        wrb_debug!("No HTTP body");
        if code == 200 {
            return Ok(vec![]);
        } else {
            return Err(Error::HttpError(code, headers, body_offset));
        }
    }

    let body = decode_http_body(&headers, &buf[body_offset..])?;
    if code == 200 {
        return Ok(body);
    } else {
        wrb_debug!(
            "HTTP Error\nHTTP code: {}\nHTTP body: {}\n",
            code,
            &String::from_utf8_lossy(&body)
        );
        return Err(Error::HttpError(code, headers, body_offset));
    }
}
