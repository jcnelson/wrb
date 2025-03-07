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
use std::convert::TryInto;
use std::io::{BufRead, Read, Write};
use std::ops::Deref;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};

use crate::ui::Error;
use crate::ui::Renderer;

use crate::util::{DEFAULT_CHAIN_ID, DEFAULT_WRB_CLARITY_VERSION, DEFAULT_WRB_EPOCH};
use crate::vm::storage::WritableWrbStore;

use clarity::vm::analysis;
use clarity::vm::ast::ASTRules;
use clarity::vm::contexts::OwnedEnvironment;
use clarity::vm::costs::LimitedCostTracker;
use clarity::vm::events::{SmartContractEventData, StacksTransactionEvent};
use clarity::vm::types::BufferLength;
use clarity::vm::types::FixedFunction;
use clarity::vm::types::FunctionType;
use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::SequenceSubtype;
use clarity::vm::types::StandardPrincipalData;
use clarity::vm::types::TupleData;
use clarity::vm::types::TypeSignature;
use clarity::vm::types::TypeSignature::SequenceType;
use clarity::vm::types::{CharType, SequenceData, UTF8Data, Value};
use clarity::vm::ClarityName;
use clarity::vm::SymbolicExpression;

use crate::vm::ClarityStorage;

use crate::vm::{
    clarity_vm::parse as clarity_parse, clarity_vm::run_analysis_free as clarity_analyze,
    ClarityVM, WRBLIB_CODE,
};

use clarity::vm::database::{HeadersDB, NULL_BURN_STATE_DB};
use clarity::vm::errors::Error as clarity_error;
use clarity::vm::errors::InterpreterError;

use stacks_common::util::get_epoch_time_ms;
use stacks_common::util::hash;
use stacks_common::util::hash::to_hex;
use stacks_common::util::hash::Hash160;
use stacks_common::util::sleep_ms;

use crate::ui::forms::WrbFormTypes;
use crate::ui::root::FrameUpdate;
use crate::ui::root::Root;
use crate::ui::scanline::Scanline;
use crate::ui::viewport::Viewport;

/// Events for the main wrb event loop
#[derive(Debug, Clone, PartialEq)]
pub enum WrbEvent {
    /// Page open
    Open,
    /// Close the event loop
    Close,
    /// The timer went off
    Timer,
    /// A resize happened
    Resize(u64, u64),
    /// A UI event happened.
    UI {
        element_type: WrbFormTypes,
        element_id: u128,
        event_payload: Value,
    },
}

impl WrbEvent {
    pub fn element_type(&self) -> u128 {
        match self {
            Self::Open => 3,
            Self::Close => 0,
            Self::Timer => 1,
            Self::Resize(_, _) => 2,
            Self::UI { element_type, .. } => element_type.as_u128(),
        }
    }

    pub fn element_id(&self) -> u128 {
        match self {
            Self::Open => u128::MAX,
            Self::Close => u128::MAX,
            Self::Timer => u128::MAX,
            Self::Resize(_, _) => u128::MAX,
            Self::UI {
                element_type: _,
                element_id,
                ..
            } => *element_id,
        }
    }

    pub fn event_type(&self) -> u128 {
        match self {
            Self::Open => 3,
            Self::Close => 0,
            Self::Timer => 1,
            Self::Resize(_, _) => 2,
            Self::UI {
                element_type,
                element_id: _,
                event_payload: _,
            } => element_type.as_u128(),
        }
    }

    pub fn event_payload(&self) -> Vec<u8> {
        match self {
            Self::Open | Self::Close | Self::Timer => Value::none()
                .serialize_to_vec()
                .expect("FATAL: could not serialize `none`"),
            Self::Resize(rows, cols) => Value::Tuple(
                TupleData::from_data(vec![
                    ("rows".into(), Value::UInt(u128::from(*rows))),
                    ("cols".into(), Value::UInt(u128::from(*cols))),
                ])
                .expect("FATAL: could not produce rows/cols tuple data"),
            )
            .serialize_to_vec()
            .expect("FATAL: could not serialize rows/cols tuple"),
            Self::UI {
                element_type,
                element_id,
                event_payload,
            } => event_payload.serialize_to_vec().expect(&format!(
                "FATAL: in element {} type {:?}: could not serialize `{:?}`",
                element_id, element_type, &event_payload
            )),
        }
    }
}

pub enum WrbFrameData {
    Root(Root),
    Update(FrameUpdate),
}

pub struct WrbRenderEventChannels {
    events: Receiver<WrbEvent>,
    frames: SyncSender<WrbFrameData>,
}

impl WrbRenderEventChannels {
    /// Try to send the next frame.
    /// If we're blocked, then return the frame.
    pub fn try_next_frame(&self, frame: Root) -> Option<Root> {
        match self.frames.try_send(WrbFrameData::Root(frame)) {
            Err(TrySendError::Full(WrbFrameData::Root(root))) => Some(root),
            _ => None,
        }
    }

    /// Send the next frame, but block.
    /// Return true if sent; false if the channel closed
    pub fn next_frame(&self, frame: Root) -> bool {
        self.frames.send(WrbFrameData::Root(frame)).is_ok()
    }

    /// Send the next frame update, but block
    /// Return true if sent; false if the channel closed
    pub fn next_frame_update(&self, frame_update: FrameUpdate) -> bool {
        self.frames.send(WrbFrameData::Update(frame_update)).is_ok()
    }

    /// Try and receive the next event
    pub fn poll_next_event(&self) -> Option<WrbEvent> {
        self.events.try_recv().ok()
    }

    /// Try and receive the next event (blocking)
    pub fn next_event(&self) -> Option<WrbEvent> {
        self.events.recv().ok()
    }
}

pub struct WrbUIEventChannels {
    events: SyncSender<WrbEvent>,
    frames: Receiver<WrbFrameData>,
}

impl WrbUIEventChannels {
    /// Send another event
    pub fn try_next_event(&self, event: WrbEvent) -> Option<WrbEvent> {
        match self.events.try_send(event) {
            Err(TrySendError::Full(event)) => Some(event),
            _ => None,
        }
    }

    /// send another event, but block
    pub fn next_event(&self, event: WrbEvent) -> bool {
        self.events.send(event).is_ok()
    }

    /// Get the next root pane to render, blocking
    pub fn next_frame(&self) -> Option<WrbFrameData> {
        self.frames.recv().ok()
    }

    /// Get the next root pane to render
    pub fn poll_next_frame(&self) -> Option<WrbFrameData> {
        self.frames.try_recv().ok()
    }

    /// Get a copy of the sender
    pub fn get_event_sender(&self) -> SyncSender<WrbEvent> {
        self.events.clone()
    }

    /// Destruct
    pub fn destruct(self) -> (SyncSender<WrbEvent>, Receiver<WrbFrameData>) {
        (self.events, self.frames)
    }
}

pub struct WrbChannels {}

impl WrbChannels {
    pub fn new() -> (WrbRenderEventChannels, WrbUIEventChannels) {
        let (events_channel_sender, events_channel_receiver) = sync_channel(1);
        let (root_channel_sender, root_channel_receiver) = sync_channel(1);

        let ui_channels = WrbUIEventChannels {
            events: events_channel_sender,
            frames: root_channel_receiver,
        };
        let render_channels = WrbRenderEventChannels {
            events: events_channel_receiver,
            frames: root_channel_sender,
        };
        (render_channels, ui_channels)
    }
}

impl Renderer {
    /// Go find the name of the event loop function, and make sure it has the right type.
    pub(crate) fn find_event_loop_function(
        &self,
        wrb_tx: &mut WritableWrbStore,
        headers_db: &dyn HeadersDB,
        main_code_id: &QualifiedContractIdentifier,
    ) -> Result<Option<String>, Error> {
        let event_loop_name = {
            let mut db = wrb_tx.get_clarity_db(headers_db, &NULL_BURN_STATE_DB);
            db.begin();
            let mut vm_env =
                OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);

            let qry = "(print (wrb-get-event-loop-name))";
            let event_loop_name_opt = self
                .run_query_code(&mut vm_env, main_code_id, qry)?
                .pop()
                .expect("FATAL: expected one result")
                .expect_optional()?
                .map(|name_val| name_val.expect_ascii())
                .transpose()?;

            let (mut db, _) = vm_env
                .destruct()
                .expect("Failed to recover database reference after executing transaction");

            db.roll_back()?;

            let Some(event_loop_name) = event_loop_name_opt else {
                return Ok(None);
            };
            event_loop_name
        };
        let event_loop_clarity_name =
            ClarityName::try_from(event_loop_name.as_str()).map_err(|_| {
                Error::Codec(format!(
                    "Invalid event loop function name '{}'",
                    &event_loop_name
                ))
            })?;

        // type-check it
        let mut analysis_db = wrb_tx.get_analysis_db();
        analysis_db.begin();
        let analysis = analysis_db
            .load_contract(main_code_id, &DEFAULT_WRB_EPOCH)
            .map_err(|e| clarity_error::Unchecked(e.err))?
            .expect(&format!(
                "FATAL: no contract analysis for main code body '{}'",
                main_code_id
            ));
        analysis_db
            .roll_back()
            .map_err(|e| clarity_error::Unchecked(e.err))?;

        let func = if let Some(f) = analysis
            .read_only_function_types
            .get(&event_loop_clarity_name)
        {
            f
        } else if let Some(f) = analysis.public_function_types.get(&event_loop_clarity_name) {
            f
        } else {
            return Err(Error::Page(format!(
                "No such event loop function '{}'",
                &event_loop_name
            )));
        };

        match func {
            FunctionType::Fixed(FixedFunction { args, returns: _ }) => {
                if args.len() != 4 {
                    return Err(Error::Page(format!("Function '{}' expects 4 arguments ((element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024)), but has {}", &event_loop_name, args.len())));
                }

                let buff1024 = SequenceType(SequenceSubtype::BufferType(
                    BufferLength::try_from(1024u32)
                        .expect("BUG: Legal Clarity buffer length marked invalid"),
                ));

                // arguments must be uints
                for (i, arg) in args.iter().enumerate() {
                    if i < 3
                        && !arg
                            .signature
                            .admits_type(&DEFAULT_WRB_EPOCH, &TypeSignature::UIntType)
                            .unwrap_or(false)
                    {
                        return Err(Error::Page(format!("Function '{}' expects uint arguments for the first three args of ((element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024))", &event_loop_name)));
                    } else if i == 3
                        && !arg
                            .signature
                            .admits_type(&DEFAULT_WRB_EPOCH, &buff1024)
                            .unwrap_or(false)
                    {
                        return Err(Error::Page(format!("Function '{}' expects (buff 1024) arguments the last arg of ((element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024))", &event_loop_name)));
                    }
                }
            }
            _ => {
                return Err(Error::Page(format!(
                    "Function '{}' is not a fixed function",
                    &event_loop_name
                )));
            }
        }

        Ok(Some(event_loop_name))
    }

    /// Go find event subscriptions.
    pub(crate) fn find_event_subscriptions(
        &self,
        wrb_tx: &mut WritableWrbStore,
        headers_db: &dyn HeadersDB,
        main_code_id: &QualifiedContractIdentifier,
    ) -> Result<HashSet<u128>, Error> {
        let mut db = wrb_tx.get_clarity_db(headers_db, &NULL_BURN_STATE_DB);
        db.begin();
        let mut vm_env = OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);

        let num_subscriptions = {
            let qry = "(print (wrb-get-num-event-subscriptions))";
            let num_subscriptions = self
                .run_query_code(&mut vm_env, main_code_id, qry)?
                .pop()
                .expect("FATAL: expected one result")
                .expect_u128()?;

            num_subscriptions
        };

        let mut subscriptions = HashSet::new();
        for idx in 0..num_subscriptions {
            let qry = format!("(print (wrb-get-event-subscription u{}))", idx);
            let event_type = self
                .run_query_code(&mut vm_env, main_code_id, &qry)?
                .pop()
                .expect("FATAL: expected one result")
                .expect_optional()?
                .expect("FATAL: index does not point to an existing event subscription")
                .expect_u128()?;

            subscriptions.insert(event_type);
        }

        let (mut db, _) = vm_env
            .destruct()
            .expect("Failed to recover database reference after executing transaction");

        db.roll_back()?;
        Ok(subscriptions)
    }

    /// Go get the event loop delay
    pub(crate) fn find_event_loop_delay(
        &self,
        wrb_tx: &mut WritableWrbStore,
        headers_db: &dyn HeadersDB,
        main_code_id: &QualifiedContractIdentifier,
    ) -> Result<u64, Error> {
        let mut db = wrb_tx.get_clarity_db(headers_db, &NULL_BURN_STATE_DB);
        db.begin();
        let mut vm_env = OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);

        let delay_val = {
            let qry = "(print (wrb-get-event-loop-time))";
            let mut delay_ms = self
                .run_query_code(&mut vm_env, main_code_id, &qry)?
                .pop()
                .expect("FATAL: expected one result")
                .expect_u128()?;

            if delay_ms > u128::from(u64::MAX) {
                delay_ms = u128::from(u64::MAX);
            }
            delay_ms as u64
        };

        let (mut db, _) = vm_env
            .destruct()
            .expect("Failed to recover database reference after executing transaction");

        db.roll_back()?;
        Ok(delay_val)
    }

    /// Run the event loop with a given event.
    /// Returns whatever the event loop function returns.
    pub(crate) fn run_one_event_loop_pass(
        &self,
        wrb_tx: &mut WritableWrbStore,
        headers_db: &dyn HeadersDB,
        main_code_id: &QualifiedContractIdentifier,
        event_handler: &str,
        event: WrbEvent,
    ) -> Result<Value, Error> {
        let mut db = wrb_tx.get_clarity_db(headers_db, &NULL_BURN_STATE_DB);
        db.begin();
        let mut vm_env = OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);

        let runner = format!(
            "(print ({} u{} u{} u{} 0x{}))",
            event_handler,
            event.element_type(),
            event.element_id(),
            event.event_type(),
            to_hex(&event.event_payload())
        );
        let res = self
            .run_query_code(&mut vm_env, main_code_id, &runner)?
            .last()
            .cloned()
            .expect("FATAL: expected one result");

        let (mut db, _) = vm_env
            .destruct()
            .expect("Failed to recover database reference after executing transaction");

        db.commit()?;
        Ok(res)
    }

    /// Link code with wrblib
    pub(crate) fn wrb_link(code: &str) -> String {
        let linked_code = format!(
            "{}\n;; ============= END OF WRBLIB ===================\n{}",
            WRBLIB_CODE, code
        );
        linked_code
    }

    /// Run the main loop in an interactive setting
    /// Returns the last thing the event loop returns.
    /// Returns None if there's no event loop function defined.
    pub fn run_page(
        &self,
        vm: &mut ClarityVM,
        compressed_input: &[u8],
        channels: WrbRenderEventChannels,
    ) -> Result<Option<Value>, Error> {
        let input = self.read_as_ascii(&mut &compressed_input[..])?;
        let linked_code = Self::wrb_link(&input);
        let main_code_id = vm.get_code_id();
        let headers_db = vm.headers_db();

        let code_hash = Hash160::from_data(compressed_input);
        let mut wrb_tx = vm.begin_page_load(&code_hash)?;

        // instantiate and run main code
        self.initialize_main(&mut wrb_tx, &headers_db, &main_code_id, &linked_code)?;
        wrb_tx.commit()?;

        let mut wrb_tx = vm.begin_page_load(&code_hash)?;

        // TODO: if any modules are declared, load them up and instantiate them here

        // set up event loop
        let event_loop_func_opt =
            self.find_event_loop_function(&mut wrb_tx, &headers_db, &main_code_id)?;
        let event_subscriptions =
            self.find_event_subscriptions(&mut wrb_tx, &headers_db, &main_code_id)?;
        let event_loop_delay =
            self.find_event_loop_delay(&mut wrb_tx, &headers_db, &main_code_id)?;

        let Some(event_loop_func) = event_loop_func_opt else {
            wrb_debug!("Running single-pass event loop");
            // done
            let mut db = wrb_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
            db.begin();
            let mut vm_env =
                OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);
            let root = self.make_root(&mut vm_env, &main_code_id)?;
            let (mut db, _) = vm_env
                .destruct()
                .expect("Failed to recover database reference after executing transaction");

            db.roll_back()?;

            let _ = channels.next_frame(root);
            wrb_tx.commit()?;
            return Ok(None);
        };

        let mut event_loop_result = None;
        let has_timer_event = event_subscriptions.contains(&WrbEvent::Timer.event_type());
        wrb_debug!(
            "Begin event loop with event_loop_delay = {}ms",
            event_loop_delay
        );

        let mut will_close = false;
        let mut root_viewports: Option<Vec<Viewport>> = None;
        while !will_close {
            let Some(next_event) = channels.next_event() else {
                break;
            };

            // if this was a request to close, then exit
            will_close = matches!(next_event, WrbEvent::Close);

            if event_subscriptions.len() > 0 {
                let event_type_u128 = next_event.event_type();
                if !event_subscriptions.contains(&event_type_u128)
                    && !matches!(next_event, WrbEvent::Open)
                {
                    wrb_debug!(
                        "Not subscribed to event type {:?}",
                        &next_event.event_type()
                    );
                    continue;
                }
            }

            wrb_debug!("Got event: {:?}", &next_event);

            // got an event we can handle
            if let Some(viewports) = root_viewports.as_ref() {
                // make a frame update
                let mut db = wrb_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
                db.begin();

                let mut vm_env =
                    OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);
                let root_update = self.make_root_update(&mut vm_env, &main_code_id, viewports)?;

                let (mut db, _) = vm_env
                    .destruct()
                    .expect("Failed to recover database reference after executing transaction");

                db.roll_back()?;

                if !channels.next_frame_update(root_update) {
                    // channel broken
                    wrb_debug!("Exiting event loop due to broken frame channel");
                    break;
                }
            } else {
                // make next whole frame
                let mut db = wrb_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
                db.begin();

                let mut vm_env =
                    OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);
                let mut root = self.make_root(&mut vm_env, &main_code_id)?;
                root.frame_delay = if has_timer_event {
                    Some(event_loop_delay)
                } else {
                    None
                };

                let viewports = root.viewports().to_vec();

                let (mut db, _) = vm_env
                    .destruct()
                    .expect("Failed to recover database reference after executing transaction");

                db.roll_back()?;

                if !channels.next_frame(root) {
                    // channel broken
                    wrb_debug!("Exiting event loop due to broken frame channel");
                    break;
                }

                // send updates from now on
                root_viewports = Some(viewports);
            }

            wrb_debug!("Running event loop pass");
            event_loop_result = match self.run_one_event_loop_pass(
                &mut wrb_tx,
                &headers_db,
                &main_code_id,
                &event_loop_func,
                next_event,
            ) {
                Ok(result) => {
                    wrb_debug!("Event loop returned: {:?}", &result);
                    Some(result)
                }
                Err(e) => {
                    wrb_warn!("Failed to run event loop: {:?}", &e);
                    break;
                }
            };
        }

        wrb_debug!("Event loop finished");

        // done
        wrb_tx.commit()?;
        Ok(event_loop_result)
    }
}
