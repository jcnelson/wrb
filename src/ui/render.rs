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
use pulldown_cmark;
use pulldown_cmark::Parser as CMParser;
use pulldown_cmark::Options as CMOpts;
use pulldown_cmark::Event as CMEvent;
use pulldown_cmark::Tag as CMTag;
use pulldown_cmark::CodeBlockKind as CMCodeBlockKind;
use pulldown_cmark::LinkType as CMLinkType;
use pulldown_cmark::HeadingLevel as CMHeadingLevel;
use pulldown_cmark::Alignment as CMAlignment;

use std::io::{BufRead, Write};
use std::ops::Deref;
use std::collections::HashSet;

use crate::ui::Renderer;
use crate::ui::Error;
use crate::ui::TableState;

use crate::storage::WritableWrbStore;
use crate::util::{DEFAULT_WRB_EPOCH, DEFAULT_WRB_CLARITY_VERSION, DEFAULT_CHAIN_ID};

use clarity::vm::ast::ASTRules;
use clarity::vm::analysis;
use clarity::vm::costs::LimitedCostTracker;
use clarity::vm::SymbolicExpression;
use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::contexts::OwnedEnvironment;
use clarity::vm::events::{
    StacksTransactionEvent,
    SmartContractEventData
};
use clarity::vm::types::StandardPrincipalData;
use clarity::vm::types::{Value, CharType, SequenceData};

use crate::vm::ClarityStorage;

use crate::vm::{
    ClarityVM,
    clarity_vm::parse as clarity_parse,
    clarity_vm::run_analysis_free as clarity_analyze
};

use clarity::vm::errors::Error as clarity_error;

use clarity::vm::database::{
    HeadersDB,
    NULL_BURN_STATE_DB,
};

use stacks_common::util::hash;

impl Renderer {
    pub fn new(max_attachment_size: usize) -> Renderer {
        Renderer {
            max_attachment_size,
            block_quote_level: 0,
            list_stack: vec![],
            table_state: None,
            footnote_labels: HashSet::new(),
        }
    }

    /// Encode a stream of bytes into an LZMA-compressed byte stream
    pub fn encode<R, W>(&self, input: &mut R, output: &mut W) -> Result<(), Error>
    where
        R: BufRead,
        W: Write
    {
        lzma_rs::lzma_compress(input, output)
            .map_err(|e| e.into())
    }

    /// Helper to encode a byte slice (LZMA-compressed)
    pub fn encode_bytes(&self, mut input: &[u8]) -> Result<Vec<u8>, Error> {
        let mut out = vec![];
        lzma_rs::lzma_compress(&mut input, &mut out)
            .map_err(Error::IOError)?;
        Ok(out)
    }

    /// Decode an attachment into bytes (written to `output`).
    /// Input must be an LZMA-compressed stream.
    /// TODO: use a bounded reader on this!
    pub fn decode<R, W>(&self, input: &mut R, output: &mut W) -> Result<(), Error>
    where
        R: BufRead,
        W: Write
    {
        lzma_rs::lzma_decompress(input, output)
            .map_err(|e| e.into())
    }

    /// Instantiate the main code 
    fn initialize_main(&self, wrb_tx: &mut WritableWrbStore, headers_db: &dyn HeadersDB, code_id: &QualifiedContractIdentifier, code: &str) -> Result<(), Error> {
        debug!("main code = '{}'", code);
        let mut main_exprs = clarity_parse(code_id, code)?;
        let analysis_result = clarity_analyze(code_id, &mut main_exprs, wrb_tx, true);
        match analysis_result {
            Ok(_) => {
                let db = wrb_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
                let mut vm_env = OwnedEnvironment::new_free(
                    true,
                    DEFAULT_CHAIN_ID,
                    db,
                    DEFAULT_WRB_EPOCH,
                );
                vm_env
                    .initialize_versioned_contract(
                        code_id.clone(),
                        DEFAULT_WRB_CLARITY_VERSION,
                        code,
                        None,
                        ASTRules::PrecheckSize
                    )?;
            }
            Err((e, _)) => {
                return Err(Error::Clarity(clarity_error::Unchecked(e.err)));
            }
        };
        Ok(())
    }

    /// Is this a wrb code tag?
    fn is_wrb_code(tag: &CMTag, check: &str) -> bool {
        match tag {
            CMTag::CodeBlock(CMCodeBlockKind::Fenced(fence)) => fence.deref() == check,
            _ => false
        }
    }

    /// Decode the decompressed attachment and find its main code and document code snippets
    fn find_code(&self, input: &str) -> Result<(String, Vec<String>), Error> {
        // set up the common-mark parser
        let mut opts = CMOpts::empty();
        opts.insert(CMOpts::ENABLE_TABLES);
        opts.insert(CMOpts::ENABLE_STRIKETHROUGH);

        let mut main_code = None;
        let mut doc_code : Vec<String> = vec![];

        let mut in_main_code = false;
        let mut in_doc_code = false;

        let mut parser = CMParser::new_ext(input, opts);
        while let Some(event) = parser.next() {
            match event {
                CMEvent::Start(tag) => {
                    if Self::is_wrb_code(&tag, "wrb:main") {
                        in_main_code = true;
                        in_doc_code = false;
                        if main_code.is_none() {
                            main_code = Some("".to_string());
                        }
                    }
                    else if Self::is_wrb_code(&tag, "wrb") {
                        in_main_code = false;
                        in_doc_code = true;
                        doc_code.push("".to_string());
                    }
                }
                CMEvent::End(tag) => {
                    if Self::is_wrb_code(&tag, "wrb:main") {
                        in_main_code = false;
                    }
                    else if Self::is_wrb_code(&tag, "wrb") {
                        in_doc_code = false;
                    }
                },
                CMEvent::Text(txt) => {
                    if in_main_code {
                        if let Some(code) = main_code.as_mut() {
                            code.push_str(txt.deref());
                        }
                    }
                    else if in_doc_code {
                        if let Some(code) = doc_code.last_mut() {
                            code.push_str(txt.deref());
                        }
                    }
                }
                _ => {}
            }
        }
        Ok((main_code.unwrap_or("".to_string()), doc_code))
    }

    /// Check the doc code.  It runs in the same contract context as the main code, but distinct
    /// from all other doc codes.
    fn analyze_doc_code<C: ClarityStorage>(&self, wrb_conn: &mut C, code_id: &QualifiedContractIdentifier, main_code: &str, doc_code: &str) -> Result<(), Error> {
        let combined_code = format!("{}\n{}", main_code, doc_code);
        let mut doc_exprs = clarity_parse(&code_id, &combined_code)?;
        analysis::run_analysis(
            code_id,
            &mut doc_exprs,
            &mut wrb_conn.get_analysis_db(),
            false,
            LimitedCostTracker::new_free(),
            DEFAULT_WRB_CLARITY_VERSION,
        ).map_err(|(e, _)| Error::Clarity(clarity_error::Unchecked(e.err)))?;
        Ok(())
    }

    /// Render a Clarity CharData
    fn render_chartype(char_data: CharType) -> String {
        match char_data {
            CharType::ASCII(ascii_data) => {
                std::str::from_utf8(&ascii_data.data)
                    .unwrap_or(&format!("{}", &ascii_data))
                    .to_string()
            }
            CharType::UTF8(utf8_data) => {
                let mut result = String::new();
                for c in utf8_data.data.iter() {
                    let next_chr_str = match std::str::from_utf8(c) {
                        Ok(s) => s.to_string(),
                        Err(_) => {
                            if c.len() > 1 {
                                format!("\\u{{{}}}", hash::to_hex(&c[..]))
                            } else {
                                format!("{}", std::ascii::escape_default(c[0]))
                            }
                        }
                    };
                    result.push_str(&next_chr_str);
                }
                result
            }
        }
    }

    /// Evaluate a piece of doc code in the context of the main contract
    fn eval_doc_code(&self, vm: &mut ClarityVM, code_id: &QualifiedContractIdentifier, doc_code: &str) -> Result<String, Error> {
        let headers_db = vm.headers_db();
        let mut wrb_tx = vm.begin_page_load();
        let db = wrb_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
        let mut vm_env = OwnedEnvironment::new_free(
            true,
            DEFAULT_CHAIN_ID,
            db,
            DEFAULT_WRB_EPOCH,
        );

        let (contract, _, _) = vm_env
            .execute_in_env(
                StandardPrincipalData::transient().into(),
                None,
                None,
                |env| {
                    env.global_context.database.get_contract(code_id)
                }
            )?;

        let (_, _, events) = vm_env
            .execute_in_env(
                StandardPrincipalData::transient().into(),
                None,
                Some(contract.contract_context),
                |env| {
                    env.eval_raw_with_rules(doc_code, ASTRules::PrecheckSize)
                }
            )?;

        let mut value_str = "".to_string();
        for event in events {
            if let StacksTransactionEvent::SmartContractEvent(event) = event {
                if event.key.1 == "print" {
                    if let Value::Sequence(SequenceData::String(char_data)) = event.value {
                        value_str.push_str(&format!("{}\n", &Self::render_chartype(char_data)));
                    }
                    else {
                        value_str.push_str(&format!("{}\n", &event.value));
                    }
                }
            }
        }
        Ok(value_str)
    }

    /// Write a link type, given the type, destination URL, and title
    fn write_linktype<W: Write>(&self, output: &mut W, linktype: &CMLinkType, dest_url: &str, title: &str) -> Result<(), Error> {
        match linktype {
            CMLinkType::Inline => {
                write!(output, "[{}]({})", title, dest_url)?;
            }
            CMLinkType::Reference => {
                write!(output, "[{}][{}]", title, dest_url)?;
            }
            CMLinkType::ReferenceUnknown => {
                write!(output, "[{}][{}]", title, dest_url)?;
            }
            CMLinkType::Collapsed => {
                write!(output, "[{}][]", title)?;
            }
            CMLinkType::CollapsedUnknown => {
                write!(output, "[{}][]", title)?;
            }
            CMLinkType::Shortcut => {
                write!(output, "[{}]", title)?;
            }
            CMLinkType::ShortcutUnknown => {
                write!(output, "[{}]", title)?;
            }
            CMLinkType::Autolink => {
                write!(output, "<{}>", dest_url)?;
            }
            CMLinkType::Email => {
                write!(output, "<{}>", dest_url)?;
            }
        }
        Ok(())
    }

    /// Write a tag
    fn write_tag<W: Write>(&mut self, start: bool, tag: &CMTag, output: &mut W) -> Result<(), Error> {
        match tag {
            CMTag::Paragraph => {
                write!(output, "\n")?;
                if !start {
                    write!(output, "\n")?;
                }
            }
            CMTag::Heading(ref heading_level, ref _frag_id, ref _classes) => {
                if start {
                    match heading_level {
                        CMHeadingLevel::H1 => {
                            write!(output, "# ")?;
                        }
                        CMHeadingLevel::H2 => {
                            write!(output, "## ")?;
                        }
                        CMHeadingLevel::H3 => {
                            write!(output, "### ")?;
                        }
                        CMHeadingLevel::H4 => {
                            write!(output, "#### ")?;
                        }
                        CMHeadingLevel::H5 => {
                            write!(output, "##### ")?;
                        }
                        CMHeadingLevel::H6 => {
                            write!(output, "###### ")?;
                        }
                    }
                }
                else {
                    write!(output, "\n")?;
                }
            }
            CMTag::BlockQuote => {
                if start {
                    self.block_quote_level += 1;
                    for _ in 0..self.block_quote_level {
                        write!(output, "> ")?;
                    }
                }
                else {
                    if self.block_quote_level > 0 {
                        self.block_quote_level -= 1;
                        if self.block_quote_level == 0 {
                            write!(output, "\n")?;
                        }
                    }
                }
            },
            CMTag::CodeBlock(ref code_block_kind) => {
                match code_block_kind {
                    CMCodeBlockKind::Indented => {
                        write!(output, "```\n")?;
                    }
                    CMCodeBlockKind::Fenced(ref code_type) => {
                        if start {
                            write!(output, "```{}\n", code_type)?;
                        }
                        else {
                            write!(output, "```\n")?;
                        }
                    }
                }
            }
            CMTag::List(ref first_num_opt) => {
                if start {
                    if self.list_stack.len() == 0 {
                        write!(output, "\n")?;
                    }
                    self.list_stack.push(first_num_opt.clone());
                }
                else {
                    self.list_stack.pop();
                    if self.list_stack.len() == 0 {
                        write!(output, "\n")?;
                    }
                }
            }
            CMTag::Item => {
                if start {
                    let depth = self.list_stack.len();
                    match self.list_stack.last_mut() {
                        Some(ref mut list_num_opt) => {
                            for _ in 0..depth {
                                write!(output, "   ")?;
                            }

                            match list_num_opt {
                                Some(ref mut ctr) => {
                                    write!(output, "{}. ", ctr)?;
                                    *ctr += 1;
                                },
                                None => {
                                    write!(output, "* ")?;
                                }
                            }
                        }
                        None => {}
                    }
                }
                else {
                    write!(output, "\n")?;
                }
            }
            CMTag::FootnoteDefinition(ref label) => {
                if start {
                    self.footnote_labels.insert(label.deref().to_string());
                }
            }
            CMTag::Table(ref alignments) => {
                if start {
                    self.table_state = Some(TableState::Header(alignments.clone()));
                }
                else {
                    self.table_state = None;
                }
            }
            CMTag::TableHead => {
                if !start {
                    write!(output, "|\n")?;
                }
            },
            CMTag::TableRow => {
                if start {
                    let new_state = match self.table_state.take() {
                        Some(TableState::Header(alignments)) => {
                            // write the separators
                            for alignment in alignments.iter() {
                                match alignment {
                                    CMAlignment::Left => write!(output, "| :--- ")?,
                                    CMAlignment::Center => write!(output, "| :---: ")?,
                                    CMAlignment::Right => write!(output, "| ---: ")?,
                                    CMAlignment::None => write!(output, "| --- ")?,
                                }
                            }
                            write!(output, "|\n")?;
                            Some(TableState::Body)
                        },
                        Some(TableState::Body) => Some(TableState::Body),
                        None => None
                    };
                    self.table_state = new_state;
                }
                else {
                    if self.table_state.is_some() {
                        write!(output, "|\n")?;
                    }
                }
            },
            CMTag::TableCell => {
                if start {
                    write!(output, "| ")?;
                }
                else {
                    write!(output, " ")?;
                }
            }
            CMTag::Emphasis => {
                write!(output, "_")?;
            }
            CMTag::Strong => {
                write!(output, "**")?;
            }
            CMTag::Strikethrough => {
                write!(output, "~~")?;
            }
            CMTag::Link(ref linktype, ref dest_url, ref title) => {
                self.write_linktype(output, linktype, dest_url.deref(), title.deref())?;
            }
            CMTag::Image(ref linktype, ref dest_url, ref title) => {
                self.write_linktype(output, linktype, dest_url.deref(), title.deref())?;
            }
        }
        Ok(())
    }
    
    /// Decode the decompressed attachment 
    pub fn eval<W: Write>(&mut self, vm: &mut ClarityVM, input: &str, output: &mut W) -> Result<(), Error> {
        self.inner_eval(vm, input, output, true)
    }

    /// Decode the decompressed attachment 
    fn inner_eval<W: Write>(&mut self, vm: &mut ClarityVM, input: &str, output: &mut W, toplevel: bool) -> Result<(), Error> {
        let main_code_id = vm.get_code_id();
        
        if toplevel {
            // top-level call.
            // set up the main code for this document if we haven't already.
            let has_main = vm.has_code(&main_code_id);
            if !has_main {
                let (main_code, doc_codes) = self.find_code(input)?;
                let headers_db = vm.headers_db();
                let mut wrb_tx = vm.begin_page_load();

                // type-check the doc codes against the main code
                for doc_code in doc_codes.iter() {
                    self.analyze_doc_code(&mut wrb_tx, &main_code_id, &main_code, doc_code)?;
                }

                // instantiate the main code
                self.initialize_main(&mut wrb_tx, &headers_db, &main_code_id, &main_code)?;
                wrb_tx.commit();
            }
        }

        // current doc code
        let mut doc_code = None;
        let mut skip_code = false;

        // set up the common-mark parser
        let mut opts = CMOpts::empty();
        opts.insert(CMOpts::ENABLE_TABLES);
        opts.insert(CMOpts::ENABLE_STRIKETHROUGH);

        let mut parser = CMParser::new_ext(input, opts);
        while let Some(event) = parser.next() {
            debug!("event: {:?}", &event);
            match event {
                CMEvent::Start(tag) => {
                    if Self::is_wrb_code(&tag, "wrb:main") {
                        // skip -- already instantiated
                        skip_code = true;
                        continue;
                    }
                    else if Self::is_wrb_code(&tag, "wrb") && toplevel {
                        // doc code -- evaluate it and parse the output as markdown
                        doc_code = Some("".to_string());
                        skip_code = true;
                    }
                    else {
                        self.write_tag(true, &tag, output)?;
                    }
                }
                CMEvent::End(tag) => {
                    if Self::is_wrb_code(&tag, "wrb:main") {
                        // skip -- already instantiated
                        skip_code = false;
                        continue;
                    }
                    else if Self::is_wrb_code(&tag, "wrb") && toplevel {
                        if let Some(doc_code) = doc_code.take() {
                            if doc_code.len() > 0 {
                                // evaluate the doc code and splice it in
                                debug!("Eval doc code '{}'", &doc_code);
                                let doc_input = self.eval_doc_code(vm, &main_code_id, &doc_code)?;
                                debug!("doc code:\n>>>>>>>\n{}\n<<<<<<", &doc_input);
                                self.inner_eval(vm, &doc_input, output, false)?;
                            }
                        }
                        skip_code = false;
                    }
                    else {
                        self.write_tag(false, &tag, output)?;
                    }
                }
                CMEvent::Text(txt) => {
                    if let Some(doc_code) = doc_code.as_mut() {
                        doc_code.push_str(txt.deref());
                    }
                    else if !skip_code {
                        write!(output, "{}", txt)?;
                    }
                }
                CMEvent::Code(code) => {
                    if !skip_code {
                        write!(output, "`{}`", code)?;
                    }
                }
                CMEvent::Html(_html) => {

                }
                CMEvent::FootnoteReference(_txt) => {

                }
                CMEvent::SoftBreak => {
                    if !skip_code {
                        write!(output, "\n")?;
                    }
                }
                CMEvent::HardBreak => {
                    if !skip_code {
                        write!(output, "\n\n")?;
                    }
                }
                CMEvent::Rule => {
                    if !skip_code {
                        write!(output, "---\n")?;
                    }
                }
                CMEvent::TaskListMarker(done) => {
                    if !skip_code {
                        if done {
                            write!(output, "[x]")?;
                        }
                        else {
                            write!(output, "[ ]")?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn eval_to_string(&mut self, vm: &mut ClarityVM, input: &str) -> Result<String, Error> {
        let mut bytes = vec![];
        self.eval(vm, input, &mut bytes)?;
        let s = std::str::from_utf8(&bytes).map_err(|_| Error::Codec("Unable to encode eval'ed markdown to String".to_string()))?;
        Ok(s.to_string())
    }
}

