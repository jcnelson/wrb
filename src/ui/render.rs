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

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::{BufRead, Read, Write};
use std::ops::Deref;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use crate::ui::Error;

use crate::util::{DEFAULT_CHAIN_ID, DEFAULT_WRB_CLARITY_VERSION, DEFAULT_WRB_EPOCH};
use crate::vm::storage::WritableWrbStore;

use clarity::vm::analysis;
use clarity::vm::analysis::CheckErrors;
use clarity::vm::ast::ASTRules;
use clarity::vm::contexts::OwnedEnvironment;
use clarity::vm::costs::LimitedCostTracker;
use clarity::vm::events::{SmartContractEventData, StacksTransactionEvent};
use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::StandardPrincipalData;
use clarity::vm::types::{CharType, SequenceData, UTF8Data, Value};
use clarity::vm::SymbolicExpression;

use crate::vm::ClarityStorage;

use crate::vm::{
    clarity_vm::parse as clarity_parse, clarity_vm::run_analysis_free as clarity_analyze,
    ClarityVM, WRBLIB_CODE,
};

use clarity::vm::database::{HeadersDB, NULL_BURN_STATE_DB};
use clarity::vm::errors::Error as clarity_error;
use clarity::vm::errors::InterpreterError;

use stacks_common::util::hash;
use stacks_common::util::hash::Hash160;
use stacks_common::util::retry::BoundReader;

use crate::ui::charbuff::CharBuff;
use crate::ui::charbuff::Color;
use crate::ui::events::WrbChannels;
use crate::ui::events::WrbEvent;
use crate::ui::events::WrbFrameData;
use crate::ui::root::{FrameUpdate, Root, SceneGraph};
use crate::ui::scanline::Scanline;
use crate::ui::viewport::Viewport;

use crate::ui::forms::WrbForm;
use crate::ui::forms::{Button, Checkbox, PrintText, RawText, TextArea, TextLine, WrbFormTypes};

pub struct Renderer {
    /// maximum wrbsite size -- a decoded string can't be longer than this
    max_attachment_size: u64,
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

    /// Decode a wrbsite into bytes (written to `output`).
    /// Input must be an LZMA-compressed stream.
    /// TODO: need a bufread bound reader
    pub fn decode<R, W>(input: &mut R, output: &mut W) -> Result<(), Error>
    where
        R: BufRead,
        W: Write,
    {
        lzma_rs::lzma_decompress(input, output).map_err(|e| e.into())
    }

    /// Decode a wrbsite into bytes
    /// Input must be an LZMA-compressed stream.
    /// TODO: need a bufread bound reader
    pub fn decode_bytes(input: &[u8]) -> Result<Vec<u8>, Error> {
        let mut output = vec![];
        Renderer::decode(&mut &input[..], &mut output)?;
        Ok(output)
    }

    /// Instantiate the main code.
    /// `code` needs to have wrblib linked into it.
    /// On success, asmart contract with the given identifier is instantiated.
    pub(crate) fn initialize_main(
        &self,
        wrb_tx: &mut WritableWrbStore,
        headers_db: &dyn HeadersDB,
        code_id: &QualifiedContractIdentifier,
        code: &str,
    ) -> Result<(), Error> {
        wrb_test_debug!("main (linked) code = '{}'", code);
        let mut main_exprs = clarity_parse(code_id, &code)?;

        wrb_debug!("Analyze contract {}", &code_id);
        match clarity_analyze(code_id, &mut main_exprs, wrb_tx, true) {
            Ok(_) => {}
            Err((e, _)) => match e.err {
                CheckErrors::ContractAlreadyExists(..) => {
                    wrb_debug!("Contract already exists: {}", &code_id);
                    return Ok(());
                }
                _ => {
                    wrb_warn!("Failed to analyze contract {}: {:?}", code_id, &e);
                    return Err(Error::Clarity(clarity_error::Unchecked(e.err)));
                }
            },
        };

        wrb_debug!("Deploy contract {}", code_id);

        let mut db = wrb_tx.get_clarity_db(headers_db, &NULL_BURN_STATE_DB);
        db.begin();
        let mut vm_env = OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);
        vm_env.initialize_versioned_contract(
            code_id.clone(),
            DEFAULT_WRB_CLARITY_VERSION,
            code,
            None,
            ASTRules::PrecheckSize,
        )?;

        let (mut db, _) = vm_env
            .destruct()
            .expect("Failed to recover database reference after executing transaction");

        db.commit()?;
        Ok(())
    }

    /// Run code to query the system state.
    /// `code` should print out Values.  These Values will be extracted and returned.
    pub(crate) fn run_query_code(
        &self,
        vm_env: &mut OwnedEnvironment,
        main_code_id: &QualifiedContractIdentifier,
        code: &str,
    ) -> Result<Vec<Value>, Error> {
        let (contract, _, _) = vm_env
            .execute_in_env(
                StandardPrincipalData::transient().into(),
                None,
                None,
                |env| env.global_context.database.get_contract(main_code_id),
            )
            .map_err(|e| {
                wrb_error!("Failed to get contract '{}': {:?}", &main_code_id, &e);
                e
            })?;

        let (_, _, events) = vm_env
            .execute_in_env(
                StandardPrincipalData::transient().into(),
                None,
                Some(contract.contract_context),
                |env| env.eval_raw_with_rules(code, ASTRules::PrecheckSize),
            )
            .map_err(|e| {
                wrb_error!("Failed to run query '{}': {:?}", &code, &e);
                e
            })?;

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

    /// Get all the viewports, arranged into a scene graph
    fn get_viewports(
        &self,
        vm_env: &mut OwnedEnvironment,
        main_code_id: &QualifiedContractIdentifier,
    ) -> Result<SceneGraph, Error> {
        let mut cursor = "none".to_string();
        let mut viewports = vec![];
        loop {
            let qry = format!("(print (wrb-get-viewports {}))", &cursor);
            let viewports_list = self
                .run_query_code(vm_env, main_code_id, &qry)?
                .pop()
                .expect("FATAL: expected one value")
                .expect_list()?;

            if viewports_list.len() == 0 {
                break;
            }

            let mut last_viewport = None;
            for vp_value in viewports_list.into_iter() {
                let viewport = Viewport::from_clarity_value(vp_value)?;
                wrb_test_debug!("loaded viewport: {:?}", &viewport);

                last_viewport = viewport.prev_viewport.clone();
                viewports.push(viewport);
            }
            if let Some(last_viewport_id) = last_viewport {
                cursor = format!("(some u{})", last_viewport_id);
            } else {
                break;
            }
        }

        viewports.reverse();
        Ok(SceneGraph::new(viewports))
    }

    /// Get the static contents
    fn get_static_ui_contents(
        &self,
        vm_env: &mut OwnedEnvironment,
        main_code_id: &QualifiedContractIdentifier,
    ) -> Result<Vec<Box<dyn WrbForm>>, Error> {
        // how many UI elements
        let qry = "(print (wrb-ui-len))";
        let num_elements = self
            .run_query_code(vm_env, main_code_id, qry)?
            .pop()
            .expect("FATAL: expected one result")
            .expect_u128()?;

        let mut ui_contents: Vec<Box<dyn WrbForm>> = Vec::with_capacity(num_elements as usize);

        for index in 0..num_elements {
            wrb_debug!("Get element {} of {}", index, num_elements);
            let qry = format!("(print (wrb-ui-element-descriptor u{}))", index);
            let ui_desc_tuple = self
                .run_query_code(vm_env, main_code_id, &qry)?
                .pop()
                .expect("FATAL: expected one result")
                .expect_optional()?
                .expect("FATAL: expected UI descriptor at defined index")
                .expect_tuple()?;

            let ui_type_value = ui_desc_tuple
                .get("type")
                .cloned()
                .expect("FATAL: expected 'type'")
                .expect_u128()?;

            let viewport_id = ui_desc_tuple
                .get("viewport")
                .cloned()
                .expect("FATAL: expected 'viewport'")
                .expect_u128()?;

            let Ok(ui_type) = WrbFormTypes::try_from(ui_type_value) else {
                wrb_warn!("Unsupported UI element type {}", ui_type_value);
                continue;
            };

            wrb_debug!("Add UI element type {:?}", &ui_type);
            match ui_type {
                WrbFormTypes::Text => {
                    // go get the text
                    let qry = format!("(print (wrb-ui-get-text-element u{}))", index);
                    let viewport_text_value = self
                        .run_query_code(vm_env, main_code_id, &qry)?
                        .pop()
                        .expect("FATAL: expected one result")
                        .expect_optional()?
                        .expect("FATAL: raw text UI element not defined at defined index");

                    let raw_text = RawText::from_clarity_value(viewport_id, viewport_text_value)?;
                    ui_contents.push(Box::new(raw_text));
                }
                WrbFormTypes::Print => {
                    // go get the print/println
                    let qry = format!("(print (wrb-ui-get-print-element u{}))", index);
                    let viewport_print_value = self
                        .run_query_code(vm_env, main_code_id, &qry)?
                        .pop()
                        .expect("FATAL: expected one result")
                        .expect_optional()?
                        .expect("FATAL: raw text UI element not defined at defined index");

                    let print_text =
                        PrintText::from_clarity_value(viewport_id, viewport_print_value)?;
                    ui_contents.push(Box::new(print_text));
                }
                WrbFormTypes::Button => {
                    // go get the button
                    let qry = format!("(print (wrb-ui-get-button-element u{}))", index);
                    let viewport_button_value = self
                        .run_query_code(vm_env, main_code_id, &qry)?
                        .pop()
                        .expect("FATAL: expected one result")
                        .expect_optional()?
                        .expect("FATAL: buttont UI element not defined at defined index");

                    let button = Button::from_clarity_value(viewport_id, viewport_button_value)?;
                    ui_contents.push(Box::new(button));
                }
                WrbFormTypes::Checkbox => {
                    // go get the checkbox
                    let qry = format!("(print (wrb-ui-get-checkbox-element u{}))", index);
                    let viewport_checkbox_value = self
                        .run_query_code(vm_env, main_code_id, &qry)?
                        .pop()
                        .expect("FATAL: expected one result")
                        .expect_optional()?
                        .expect("FATAL: checkbox UI element not defined at defined index");

                    let checkbox =
                        Checkbox::from_clarity_value(viewport_id, viewport_checkbox_value)?;
                    ui_contents.push(Box::new(checkbox));
                }
                WrbFormTypes::TextLine => {
                    // go get the textline
                    let qry = format!("(print (wrb-ui-get-textline-element u{}))", index);
                    let viewport_textline_value = self
                        .run_query_code(vm_env, main_code_id, &qry)?
                        .pop()
                        .expect("FATAL: expected one result")
                        .expect_optional()?
                        .expect("FATAL: textline UI element not defined at defined index");

                    let textline =
                        TextLine::from_clarity_value(viewport_id, viewport_textline_value)?;
                    ui_contents.push(Box::new(textline));
                }
                WrbFormTypes::TextArea => {
                    // go get the textarea
                    let qry = format!("(print (wrb-ui-get-textarea-element u{}))", index);
                    let viewport_textarea_value = self
                        .run_query_code(vm_env, main_code_id, &qry)?
                        .pop()
                        .expect("FATAL: expected one result")
                        .expect_optional()?
                        .expect("FATAL: textline UI element not defined at defined index");

                    let textarea =
                        TextArea::from_clarity_value(viewport_id, viewport_textarea_value)?;
                    ui_contents.push(Box::new(textarea));
                }
            }
        }
        Ok(ui_contents)
    }

    /// Get the dynamic contents for each viewport
    fn get_dynamic_ui_contents(
        &self,
        vm_env: &mut OwnedEnvironment,
        main_code_id: &QualifiedContractIdentifier,
        viewports: &[Viewport],
    ) -> Result<Vec<Box<dyn WrbForm>>, Error> {
        // get the elements in viewport order
        let mut ui_contents: Vec<Box<dyn WrbForm>> = vec![];
        for vp in viewports {
            let qry = format!("(print (wrb-dynamic-ui-index-start u{}))", vp.id);
            let index_start = self
                .run_query_code(vm_env, main_code_id, &qry)?
                .pop()
                .expect("FATAL: expected one result")
                .expect_u128()?;

            let qry = format!("(print (wrb-dynamic-ui-index-end u{}))", vp.id);
            let index_end = self
                .run_query_code(vm_env, main_code_id, &qry)?
                .pop()
                .expect("FATAL: expected one result")
                .expect_u128()?;

            wrb_debug!(
                "Dynamic viewport id={} UI indexes {}..{}",
                vp.id,
                index_start,
                index_end
            );
            for ui_index in index_start..index_end {
                let qry = format!("(print (wrb-dynamic-ui-pointer u{} u{}))", vp.id, ui_index);
                let Some(ptr_tuple) = self
                    .run_query_code(vm_env, main_code_id, &qry)?
                    .pop()
                    .expect("FATAL: expected one result")
                    .expect_optional()?
                    .map(|tuple_value| tuple_value.expect_tuple())
                    .transpose()?
                else {
                    continue;
                };

                let ui_type = ptr_tuple
                    .get("type")
                    .cloned()
                    .expect("missing `type`")
                    .expect_u128()?;

                let ui_index = ptr_tuple
                    .get("ui-index")
                    .cloned()
                    .expect("missing `ui-index`")
                    .expect_u128()?;

                let Ok(ui_type) = WrbFormTypes::try_from(ui_type) else {
                    wrb_warn!("Unsupported UI element type {}", ui_type);
                    continue;
                };
                match ui_type {
                    WrbFormTypes::Text => {
                        // go get the text
                        let qry =
                            format!("(print (wrb-dynamic-ui-get-text-element u{}))", ui_index);
                        let viewport_text_value = self
                            .run_query_code(vm_env, main_code_id, &qry)?
                            .pop()
                            .expect("FATAL: expected one result")
                            .expect_optional()?
                            .expect("FATAL: raw text UI element not defined at defined index");

                        let raw_text = RawText::from_clarity_value(vp.id, viewport_text_value)?;
                        ui_contents.push(Box::new(raw_text));
                    }
                    WrbFormTypes::Print => {
                        // go get the print/println
                        let qry =
                            format!("(print (wrb-dynamic-ui-get-print-element u{}))", ui_index);
                        let viewport_print_value = self
                            .run_query_code(vm_env, main_code_id, &qry)?
                            .pop()
                            .expect("FATAL: expected one result")
                            .expect_optional()?
                            .expect("FATAL: raw text UI element not defined at defined index");

                        let print_text =
                            PrintText::from_clarity_value(vp.id, viewport_print_value)?;
                        ui_contents.push(Box::new(print_text));
                    }
                    _ => {
                        wrb_warn!("No logic to render UI element type {:?} (index {}) -- use the specific method to do this.", ui_type, ui_index);
                    }
                }
            }
        }
        Ok(ui_contents)
    }

    /// Compute the root pane from scratch
    pub(crate) fn make_root(
        &self,
        vm_env: &mut OwnedEnvironment,
        main_code_id: &QualifiedContractIdentifier,
    ) -> Result<Root, Error> {
        let scenegraph = self.get_viewports(vm_env, main_code_id)?;

        let qry = "(print (wrb-get-root))";
        let mut root = self
            .run_query_code(vm_env, main_code_id, qry)?
            .pop()
            .map(|root_value| {
                let root_tuple = root_value.expect_tuple()?;
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

                let root_res: Result<_, Error> = Ok(Root::new(rows, cols, scenegraph));
                root_res
            })
            .expect("FATAL: `wrb-get-root` failed to produce output")?;

        let static_ui_contents = self.get_static_ui_contents(vm_env, main_code_id)?;
        let dynamic_ui_contents =
            self.get_dynamic_ui_contents(vm_env, main_code_id, root.viewports())?;
        root.set_all_forms(static_ui_contents, dynamic_ui_contents)?;
        Ok(root)
    }

    /// Compute new data for a root
    pub(crate) fn make_root_update(
        &self,
        vm_env: &mut OwnedEnvironment,
        main_code_id: &QualifiedContractIdentifier,
        viewports: &[Viewport],
    ) -> Result<FrameUpdate, Error> {
        let dynamic_ui_contents = self.get_dynamic_ui_contents(vm_env, main_code_id, viewports)?;
        Ok(FrameUpdate {
            new_contents: dynamic_ui_contents,
        })
    }

    /// Decode an LZMA input stream into an ASCII string, throwing an error if it's not actually an
    /// ASCII string
    pub(crate) fn read_as_ascii<R: Read + BufRead>(
        &self,
        compressed_input: &mut R,
    ) -> Result<String, Error> {
        let mut decompressed_code = vec![];
        Self::decode(compressed_input, &mut decompressed_code)?;
        let input = std::str::from_utf8(&decompressed_code)
            .map_err(|_| Error::Codec("Compressed bytes did not decode to a utf8 string".into()))?;
        if !input.is_ascii() {
            return Err(Error::Codec("Expected ASCII string".into()));
        }
        Ok(input.to_string())
    }

    /// Decode the decompressed bytes into Clarity code, run it, and evaluate it into a root
    /// pane.  Does one pass of the event loop and returns the single Root
    pub fn eval_root(&self, vm: &mut ClarityVM, compressed_input: &[u8]) -> Result<Root, Error> {
        let (render_channels, ui_channels) = WrbChannels::new();
        ui_channels.next_event(WrbEvent::Close);
        self.run_page(vm, compressed_input, render_channels)?;
        let frame_data = ui_channels
            .poll_next_frame()
            .ok_or(Error::Event("Failed to poll next frame".into()))?;
        match frame_data {
            WrbFrameData::Root(root) => Ok(root),
            _ => Err(Error::Event("Did not receive root".into())),
        }
    }

    pub fn eval_to_charbuff(
        &mut self,
        vm: &mut ClarityVM,
        compressed_input: &[u8],
    ) -> Result<CharBuff, Error> {
        let mut root = self.eval_root(vm, compressed_input)?;
        let buff = root.refresh();
        Ok(buff)
    }

    pub fn eval_to_scanlines(
        &mut self,
        vm: &mut ClarityVM,
        compressed_input: &[u8],
    ) -> Result<Vec<Scanline>, Error> {
        let buff = self.eval_to_charbuff(vm, compressed_input)?;
        let scanlines = Scanline::compile(&buff);
        Ok(scanlines)
    }

    pub fn scanlines_into_term_string(scanlines: Vec<Scanline>) -> String {
        let mut output = "".to_string();
        for sl in scanlines {
            output.push_str(&sl.into_term_code());
        }
        output
    }

    pub fn scanlines_into_text(scanlines: Vec<Scanline>) -> String {
        let mut output = "".to_string();
        for sl in scanlines {
            output.push_str(&sl.into_text());
        }
        output
    }

    pub fn eval_to_string(
        &mut self,
        vm: &mut ClarityVM,
        compressed_input: &[u8],
    ) -> Result<String, Error> {
        let scanlines = self.eval_to_scanlines(vm, compressed_input)?;
        Ok(Self::scanlines_into_term_string(scanlines))
    }

    pub fn eval_to_text(
        &mut self,
        vm: &mut ClarityVM,
        compressed_input: &[u8],
    ) -> Result<String, Error> {
        let scanlines = self.eval_to_scanlines(vm, compressed_input)?;
        Ok(Self::scanlines_into_text(scanlines))
    }
}
