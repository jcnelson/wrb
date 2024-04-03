// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2022 Stacks Open Internet Foundation
// Copyright (C) 2022 Jude Nelson
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

use lzma_rs;

use std::collections::HashSet;
use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::ops::Deref;

use crate::ui::Error;
use crate::ui::Renderer;

use crate::vm::storage::WritableWrbStore;
use crate::util::{DEFAULT_CHAIN_ID, DEFAULT_WRB_CLARITY_VERSION, DEFAULT_WRB_EPOCH};

use clarity::vm::analysis;
use clarity::vm::ast::ASTRules;
use clarity::vm::contexts::OwnedEnvironment;
use clarity::vm::costs::LimitedCostTracker;
use clarity::vm::events::{SmartContractEventData, StacksTransactionEvent};
use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::StandardPrincipalData;
use clarity::vm::types::{CharType, SequenceData, Value, UTF8Data};
use clarity::vm::SymbolicExpression;

use crate::vm::ClarityStorage;

use crate::vm::{
    clarity_vm::parse as clarity_parse, clarity_vm::run_analysis_free as clarity_analyze,
    ClarityVM, WRBLIB_CODE,
};

use clarity::vm::errors::Error as clarity_error;
use clarity::vm::errors::InterpreterError;
use clarity::vm::database::{HeadersDB, NULL_BURN_STATE_DB};

use stacks_common::util::hash;
use stacks_common::util::hash::Hash160;
use stacks_common::util::retry::BoundReader;

use crate::ui::root::Root;
use crate::ui::scanline::Scanline;
use crate::ui::viewport::Viewport;
use crate::ui::charbuff::Color;

/// UI type constants
const UI_TYPE_TEXT : u128 = 0;
const UI_TYPE_PRINT : u128 = 1;

trait ValueExtensions {
    fn expect_utf8(self) -> Result<String, clarity_error>;
}

impl ValueExtensions for Value {
    fn expect_utf8(self) -> Result<String, clarity_error> {
        if let Value::Sequence(SequenceData::String(CharType::UTF8(UTF8Data { data }))) = self {
            let mut s = String::new();
            // each item in data is a code point
            for val_bytes in data.into_iter() {
                let val_4_bytes : [u8; 4] = match val_bytes.len() {
                    0 => [0, 0, 0, 0],
                    1 => [0, 0, 0, val_bytes[0]],
                    2 => [0, 0, val_bytes[0], val_bytes[1]],
                    3 => [0, val_bytes[0], val_bytes[1], val_bytes[2]],
                    4 => [val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]],
                    _ => {
                        // invalid
                        s.push_str(&char::REPLACEMENT_CHARACTER.to_string());
                        continue;
                    }
                };
                let val_u32 = u32::from_be_bytes(val_4_bytes);
                let c = char::from_u32(val_u32).unwrap_or(char::REPLACEMENT_CHARACTER);
                s.push_str(&c.to_string());
            }
            Ok(s)
        } else {
            Err(clarity_error::Interpreter(InterpreterError::Expect("expected utf8 string".into())).into())
        }
    }
}

/// UI command to add text to a viewport
struct RawText {
    viewport_id: u128,
    col: u64,
    row: u64,
    bg_color: Color,
    fg_color: Color,
    text: String
}

impl RawText {
    pub fn from_clarity_value(viewport_id: u128, v: Value) -> Result<Self, Error> {
        let text_tuple = v.expect_tuple()?;
        let text = text_tuple
            .get("text")
            .cloned()
            .expect("FATAL: no `text`")
            .expect_utf8()?;

        let col = text_tuple
            .get("col")
            .cloned()
            .expect("FATAL: no `col`")
            .expect_u128()?;

        let row = text_tuple
            .get("row")
            .cloned()
            .expect("FATAL: no `row`")
            .expect_u128()?;

        let bg_color_u128 = text_tuple
            .get("bg-color")
            .cloned()
            .expect("FATAL: no `bg-color`")
            .expect_u128()?
            // truncate
            & 0xffffffffu128;

        
        let fg_color_u128 = text_tuple
            .get("fg-color")
            .cloned()
            .expect("FATAL: no `fg-color`")
            .expect_u128()?
            // trunate
            &0xffffffffu128;

        let bg_color : Color = u32::try_from(bg_color_u128).expect("infallible").into();
        let fg_color : Color = u32::try_from(fg_color_u128).expect("infallible").into();

        Ok(RawText {
            viewport_id,
            col: u64::try_from(col).map_err(|_| Error::Codec("Invalid 'col' value".into()))?,
            row: u64::try_from(row).map_err(|_| Error::Codec("Invalid 'row' value".into()))?,
            bg_color,
            fg_color,
            text
        })
    }

    pub fn render(&self, root: &mut Root) -> Result<(), Error> {
        let Some(viewport) = root.viewport_mut(self.viewport_id) else {
            return Err(Error::NoViewport(self.viewport_id));
        };
        viewport.print_to(self.col, self.row, self.bg_color, self.fg_color, &self.text);
        Ok(())
    }
}

/// UI command to print text to a viewport
struct PrintText {
    viewport_id: u128,
    // (column, row)
    cursor: Option<(u64, u64)>,
    bg_color: Color,
    fg_color: Color,
    text: String,
    newline: bool
}

impl PrintText {
    pub fn from_clarity_value(viewport_id: u128, v: Value) -> Result<Self, Error> {
        let text_tuple = v.expect_tuple()?;
        let text = text_tuple
            .get("text")
            .cloned()
            .expect("FATAL: no `text`")
            .expect_utf8()?;

        let cursor = match text_tuple.get("cursor").cloned().expect("FATAL: no `cursor`").expect_optional()? {
            Some(cursor_tuple_value) => {
                let cursor_tuple = cursor_tuple_value.expect_tuple()?;
                let col = cursor_tuple.get("col").cloned().expect("FATAL: no `col`").expect_u128()?;
                let row = cursor_tuple.get("row").cloned().expect("FATAL: no `row`").expect_u128()?;
                Some((u64::try_from(col).expect("col too big"), u64::try_from(row).expect("row too big")))
            }
            None => None
        };

        let bg_color_u128 = text_tuple
            .get("bg-color")
            .cloned()
            .expect("FATAL: no `bg-color`")
            .expect_u128()?
            // truncate
            & 0xffffffffu128;
        
        let fg_color_u128 = text_tuple
            .get("fg-color")
            .cloned()
            .expect("FATAL: no `fg-color`")
            .expect_u128()?
            // trunate
            &0xffffffffu128;

        let newline = text_tuple
            .get("newline")
            .cloned()
            .expect("FATAL: no `newline`")
            .expect_bool()?;

        let bg_color : Color = u32::try_from(bg_color_u128).expect("infallible").into();
        let fg_color : Color = u32::try_from(fg_color_u128).expect("infallible").into();

        Ok(PrintText {
            viewport_id,
            cursor,
            bg_color,
            fg_color,
            text,
            newline
        })
    }

    pub fn render(&self, root: &mut Root, cursor: (u64, u64)) -> Result<(u64, u64), Error> {
        let Some(viewport) = root.viewport_mut(self.viewport_id) else {
            return Err(Error::NoViewport(self.viewport_id));
        };
        let cursor = self.cursor.clone().unwrap_or(cursor);
        test_debug!("Print '{}' at {:?}", &self.text, &cursor);
        if self.newline {
            Ok(viewport.println(cursor.0, cursor.1, self.bg_color, self.fg_color, &self.text))
        }
        else {
            Ok(viewport.print(cursor.0, cursor.1, self.bg_color, self.fg_color, &self.text))
        }
    }
}

/// UI element types
enum UIContent {
    RawText(RawText),
    PrintText(PrintText),
}

impl Renderer {
    pub fn new(max_attachment_size: u64) -> Renderer {
        Renderer {
            max_attachment_size,
        }
    }

    /// Encode a stream of bytes into an LZMA-compressed byte stream
    pub fn encode<R, W>(input: &mut R, output: &mut W) -> Result<(), Error>
    where
        R: BufRead,
        W: Write,
    {
        lzma_rs::lzma_compress(input, output).map_err(|e| e.into())
    }

    /// Helper to encode a byte slice (LZMA-compressed)
    pub fn encode_bytes(mut input: &[u8]) -> Result<Vec<u8>, Error> {
        let mut out = vec![];
        lzma_rs::lzma_compress(&mut input, &mut out).map_err(Error::IOError)?;
        Ok(out)
    }

    /// Decode an attachment into bytes (written to `output`).
    /// Input must be an LZMA-compressed stream.
    /// TODO: need a bufread bound reader
    pub fn decode<R, W>(&self, input: &mut R, output: &mut W) -> Result<(), Error>
    where
        R: BufRead,
        W: Write,
    {
        lzma_rs::lzma_decompress(input, output).map_err(|e| e.into())
    }

    /// Instantiate the main code
    fn initialize_main(
        &self,
        wrb_tx: &mut WritableWrbStore,
        headers_db: &dyn HeadersDB,
        code_id: &QualifiedContractIdentifier,
        code: &str,
    ) -> Result<(), Error> {
        debug!("main (linked) code = '{}'", code);
        let mut main_exprs = clarity_parse(code_id, &code)?;
        let analysis_result = clarity_analyze(code_id, &mut main_exprs, wrb_tx, true);
        match analysis_result {
            Ok(_) => {
                let db = wrb_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
                let mut vm_env =
                    OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);
                vm_env.initialize_versioned_contract(
                    code_id.clone(),
                    DEFAULT_WRB_CLARITY_VERSION,
                    &code,
                    None,
                    ASTRules::PrecheckSize,
                )?;
            }
            Err((e, _)) => {
                return Err(Error::Clarity(clarity_error::Unchecked(e.err)));
            }
        };
        Ok(())
    }

    /// Run code to query the system state.
    /// `code` should print out Values.  These Values will be extracted and returned.
    fn run_query_code(
        &self,
        vm_env: &mut OwnedEnvironment,
        main_code_id: &QualifiedContractIdentifier,
        code: &str,
    ) -> Result<Vec<Value>, Error> {
        let (contract, _, _) = vm_env.execute_in_env(
            StandardPrincipalData::transient().into(),
            None,
            None,
            |env| env.global_context.database.get_contract(main_code_id),
        )?;

        let (_, _, events) = vm_env.execute_in_env(
            StandardPrincipalData::transient().into(),
            None,
            Some(contract.contract_context),
            |env| env.eval_raw_with_rules(code, ASTRules::PrecheckSize),
        )?;

        let values = events
            .into_iter()
            .filter_map(|event| {
                if let StacksTransactionEvent::SmartContractEvent(event) = event {
                    if event.key.1 == "print" {
                        Some(event.value)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        Ok(values)
    }

    /// Get all the viewports
    fn get_viewports(
        &self,
        vm_env: &mut OwnedEnvironment,
        main_code_id: &QualifiedContractIdentifier,
    ) -> Result<Vec<Viewport>, Error> {
        let qry = "(print (wrb-get-viewports))";
        let viewports_list = self.run_query_code(vm_env, main_code_id, qry)?
            .pop()
            .expect("FATAL: expected one value")
            .expect_list()?;

        let mut viewports = vec![];
        for vp_value in viewports_list.into_iter() {
            let viewport = Viewport::from_clarity_value(vp_value)?;
            viewports.push(viewport);
        }
        Ok(viewports)
    }

    /// Get a viewport's contents 
    fn get_ui_contents(&self, vm_env: &mut OwnedEnvironment, main_code_id: &QualifiedContractIdentifier) -> Result<Vec<UIContent>, Error> {
        // how many UI elements
        let qry = "(print (wrb-ui-len))";
        let num_elements = self.run_query_code(vm_env, main_code_id, qry)?
            .pop()
            .expect("FATAL: expected one result")
            .expect_u128()?;
        
        let mut ui_contents = Vec::with_capacity(num_elements as usize);

        for index in 0..num_elements {
            let qry = format!("(print (wrb-ui-element-descriptor u{}))", index);
            let ui_desc_tuple = self.run_query_code(vm_env, main_code_id, &qry)?
                .pop()
                .expect("FATAL: expected one result")
                .expect_optional()?
                .expect("FATAL: expected UI descriptor at defined index")
                .expect_tuple()?;

            let ui_type = ui_desc_tuple
                .get("type")
                .cloned()
                .expect("FATAL: expected 'type'")
                .expect_u128()?;

            let viewport_id = ui_desc_tuple
                .get("viewport")
                .cloned()
                .expect("FATAL: expected 'viewport'")
                .expect_u128()?;

            if ui_type == UI_TYPE_TEXT {
                // go get the text 
                let qry = format!("(print (wrb-ui-get-text-element u{}))", index);
                let viewport_text_value = self.run_query_code(vm_env, main_code_id, &qry)?
                    .pop()
                    .expect("FATAL: expected one result")
                    .expect_optional()?
                    .expect("FATAL: raw text UI element not defined at defined index");

                let raw_text = RawText::from_clarity_value(viewport_id, viewport_text_value)?;
                ui_contents.push(UIContent::RawText(raw_text));
            }
            else if ui_type == UI_TYPE_PRINT {
                // go get the print/println
                let qry = format!("(print (wrb-ui-get-print-element u{}))", index);
                let viewport_print_value = self.run_query_code(vm_env, main_code_id, &qry)?
                    .pop()
                    .expect("FATAL: expected one result")
                    .expect_optional()?
                    .expect("FATAL: raw text UI element not defined at defined index");

                let print_text = PrintText::from_clarity_value(viewport_id, viewport_print_value)?;
                ui_contents.push(UIContent::PrintText(print_text));
            }
            else {
                warn!("Unsupported UI type {} (index {})", ui_type, index);
            }
        }
        Ok(ui_contents)
    }

    /// Get the root pane
    fn get_root(
        &self,
        vm_env: &mut OwnedEnvironment,
        main_code_id: &QualifiedContractIdentifier,
    ) -> Result<Root, Error> {
        let viewports = self.get_viewports(vm_env, main_code_id)?;

        let qry = "(print (wrb-get-root))";
        let mut root = self
            .run_query_code(vm_env, main_code_id, qry)?
            .pop()
            .map(|root_value| {
                let root_tuple = root_value.expect_tuple()?;
                let cols: u64 = root_tuple
                    // get `cols` value, which is a uint
                    .get("cols")
                    .cloned()
                    .expect("missing cols")
                    // unwrap to a u128
                    .expect_u128()?
                    // convert to u64
                    .try_into()
                    .expect("too many cols");

                let rows: u64 = root_tuple
                    // get `rows` value, which is a uint
                    .get("rows")
                    .cloned()
                    .expect("missing rows")
                    // unwrap to a u128
                    .expect_u128()?
                    // convert to u64
                    .try_into()
                    .expect("too many rows");

                let root_res : Result<_, Error> = Ok(Root::new(cols, rows, viewports));
                root_res
            })
            .expect("FATAL: `wrb-get-root` failed to produce output")
            .unwrap_or(Root::new(80, 24, vec![]));

        let ui_contents = self.get_ui_contents(vm_env, main_code_id)?;
        let mut viewport_cursors = HashMap::new();
        for ui_content in ui_contents {
            match ui_content {
                UIContent::RawText(raw_text) => {
                    raw_text.render(&mut root)?;
                }
                UIContent::PrintText(print_text) => {
                    let cursor = viewport_cursors.get(&print_text.viewport_id).cloned().unwrap_or((0, 0));
                    let new_cursor = print_text.render(&mut root, cursor)?;
                    viewport_cursors.insert(print_text.viewport_id, new_cursor);
                }
            }
        }
        Ok(root)
    }

    /// Decode an LZMA input stream into an ASCII string, throwing an error if it's not actually an
    /// ASCII string
    fn read_as_ascii<R: Read + BufRead>(&self, compressed_input: &mut R) -> Result<String, Error> {
        let mut decompressed_code = vec![];
        self.decode(compressed_input, &mut decompressed_code)?;
        let input = std::str::from_utf8(&decompressed_code)
            .map_err(|_| Error::Codec("Compressed bytes did not decode to a utf8 string".into()))?;
        if !input.is_ascii() {
            return Err(Error::Codec("Expected ASCII string".into()));
        }
        Ok(input.to_string())
    }

    /// Decode the decompressed attachment into Clarity code, run it, and evaluate it into a root
    /// pane
    fn eval_root(&self, vm: &mut ClarityVM, compressed_input: &[u8]) -> Result<Root, Error> {
        let input = self.read_as_ascii(&mut &compressed_input[..])?;
        let linked_code = format!(
            "{}\n;; ============= END OF WRBLIB ===================\n{}",
            WRBLIB_CODE, input
        );
        let main_code_id = vm.get_code_id();
        let headers_db = vm.headers_db();
        let code_hash = Hash160::from_data(compressed_input);
        let mut wrb_tx = vm.begin_page_load(&code_hash)?;

        // instantiate and run main code
        self.initialize_main(&mut wrb_tx, &headers_db, &main_code_id, &linked_code)?;

        // read out UI components
        let db = wrb_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
        let mut vm_env = OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);

        let root = self.get_root(&mut vm_env, &main_code_id)?;
        wrb_tx.commit()?;
        Ok(root)
    }

    pub fn eval_to_string(
        &mut self,
        vm: &mut ClarityVM,
        compressed_input: &[u8],
    ) -> Result<String, Error> {
        let mut root = self.eval_root(vm, compressed_input)?;
        let buff = root.refresh();
        let scanlines = Scanline::compile(&buff);
        let mut output = "".to_string();
        for sl in scanlines {
            output.push_str(&sl.into_term_code());
        }
        Ok(output)
    }
    
    pub fn eval_to_text(
        &mut self,
        vm: &mut ClarityVM,
        compressed_input: &[u8],
    ) -> Result<String, Error> {
        let mut root = self.eval_root(vm, compressed_input)?;
        let buff = root.refresh();
        let scanlines = Scanline::compile(&buff);
        let mut output = "".to_string();
        for sl in scanlines {
            output.push_str(&sl.into_text())
        }
        Ok(output)
    }
}
