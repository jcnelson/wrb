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

use std::thread;
use crate::core;
use crate::ui;
use crate::ui::Renderer;
use crate::ui::root::Root;
use crate::vm::ClarityVM;
use crate::vm::ClarityStorage;
use crate::ui::events::*;
use crate::ui::scanline::Scanline;
use std::fs;

use stacks_common::util::hash::to_hex;

use clarity::vm::database::NULL_BURN_STATE_DB;
use clarity::vm::contexts::OwnedEnvironment;
use clarity::vm::Value;

use crate::util::DEFAULT_CHAIN_ID;
use crate::util::DEFAULT_WRB_EPOCH;

use stacks_common::util::hash::Hash160;

fn run_page(mut vm: ClarityVM, renderer: Renderer, code: &str, events: Vec<WrbEvent>) -> Result<(Vec<WrbFrameData>, Option<Value>), ui::Error> {
    let (render_channels, ui_channels) = WrbChannels::new();
    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let handle = thread::spawn(move || {
        renderer.run_page(&mut vm, &bytes, render_channels)
    });

    let mut frames = vec![];
    for event in events.into_iter() {
        wrb_test_debug!("Send event {:?}", &event);
        ui_channels.next_event(event);
        let root = ui_channels.next_frame().unwrap();
        frames.push(root);
    }

    handle.join().unwrap()
        .map(|value_opt| (frames, value_opt))
}

#[test]
fn test_wrb_event_loop_setup() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-event-loop-setup";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
(define-public (main (element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024)))
    (ok {
        msg: "ran event loop",
        element-type: (+ u1 element-type),
        element-id: (+ u1 element-id),
        event-type: (+ u1 event-type),
        event-payload: event-payload
    }))

(wrb-event-loop "main")
(wrb-event-subscribe WRB_EVENT_CLOSE)
(wrb-event-subscribe WRB_EVENT_TIMER)
(wrb-event-loop-time u100)
"#;

    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();
    let linked_code = Renderer::wrb_link(&code);

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let renderer = Renderer::new(1_000_000_000);
    
    let main_code_id = vm.get_code_id();
    let headers_db = vm.headers_db();
    let code_hash = Hash160::from_data(&bytes);
    let mut wrb_tx = vm.begin_page_load(&code_hash).unwrap();
        
    renderer.initialize_main(&mut wrb_tx, &headers_db, &main_code_id, &linked_code).unwrap();

    assert_eq!(renderer.find_event_loop_function(&mut wrb_tx, &headers_db, &main_code_id).unwrap(), Some("main".to_string()));

    let events = renderer.find_event_subscriptions(&mut wrb_tx, &headers_db, &main_code_id).unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.contains(&0));
    assert!(events.contains(&1));

    assert_eq!(renderer.find_event_loop_delay(&mut wrb_tx, &headers_db, &main_code_id).unwrap(), 100);
}

#[test]
fn test_wrb_stateful_event_loop() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-stateful-event-loop";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
(define-data-var event-count uint u0)
(define-public (main (element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024)))
    (let (
        (count (var-get event-count))
    )
    (var-set event-count (+ u1 count))
    (ok (var-get event-count))))

(wrb-event-loop "main")
(wrb-event-subscribe WRB_EVENT_CLOSE)
(wrb-event-subscribe WRB_EVENT_TIMER)
"#;

    let vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let renderer = Renderer::new(1_000_000_000);

    let (_frames, value_opt) = run_page(vm, renderer, code, vec![WrbEvent::Timer, WrbEvent::Timer, WrbEvent::Timer, WrbEvent::Close]).unwrap();

    assert_eq!(value_opt.unwrap().expect_result_ok().unwrap().expect_u128().unwrap(), 4);
}

#[test]
fn test_render_dynamic_text() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-render-dynamic-text";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }
    
    let code = r#"
(wrb-root u1 u60)
(wrb-viewport u0 u0 u0 u1 u60)

(define-data-var event-count uint u0)
(define-public (main (element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024)))
    (let (
        (count (var-get event-count))
    )
    (var-set event-count (+ u1 count))
    (try! (wrb-viewport-clear u0))
    (try! (wrb-txt u0 u0 count u0 (buff-to-uint-le 0xffffff) (concat u"hello world: " (int-to-utf8 count))))
    (ok (var-get event-count))))

(wrb-event-loop "main")
(wrb-event-subscribe WRB_EVENT_CLOSE)
(wrb-event-subscribe WRB_EVENT_TIMER)
"#;

    let vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let renderer = Renderer::new(1_000_000_000);

    let (frames, _value_opt) = run_page(vm, renderer, code, vec![WrbEvent::Timer, WrbEvent::Timer, WrbEvent::Timer, WrbEvent::Timer, WrbEvent::Close]).unwrap();

    let expected_texts = vec![
        "",
        "hello world: 0",
        " hello world: 1",
        "  hello world: 2",
        "   hello world: 3",
    ];
    let mut root = None;
    for (i, frame) in frames.into_iter().enumerate() {
        match frame {
            WrbFrameData::Root(mut frame) => {
                let chars = frame.render();
                let scanlines = Scanline::compile(&chars);
                let term_text = Renderer::scanlines_into_term_string(scanlines.clone());
                let test_text = Renderer::scanlines_into_text(scanlines.clone());

                println!("{}", &term_text);
                assert_eq!(test_text.trim_end(), expected_texts[i]);
                root = Some(frame);
            }
            WrbFrameData::Update(update) => {
                let mut frame = root.unwrap();
                frame.update_forms(update).unwrap();
                
                let chars = frame.render();
                let scanlines = Scanline::compile(&chars);
                let term_text = Renderer::scanlines_into_term_string(scanlines.clone());
                let test_text = Renderer::scanlines_into_text(scanlines.clone());

                println!("{}", &term_text);
                assert_eq!(test_text.trim_end(), expected_texts[i]);
                root = Some(frame);

            }
        }
    }
}


