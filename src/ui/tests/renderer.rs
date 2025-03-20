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

use crate::core;
use crate::ui::Renderer;
use crate::vm::ClarityStorage;
use crate::vm::ClarityVM;

use crate::util::DEFAULT_CHAIN_ID;
use crate::DEFAULT_WRB_EPOCH;

use clarity::vm::contexts::OwnedEnvironment;
use clarity::vm::database::NULL_BURN_STATE_DB;
use clarity::vm::Value;

use crate::ui::Error;

use std::fs;

use stacks_common::util::hash::to_hex;
use stacks_common::util::hash::Hash160;

impl Renderer {
    pub fn run_test_query_code(
        &mut self,
        vm: &mut ClarityVM,
        code: &str,
    ) -> Result<Vec<Value>, Error> {
        let main_code_id = vm.initialize_app(code)?;

        let headers_db = vm.headers_db();
        let mut wrb_tx = vm.begin_page_load()?;

        let mut db = wrb_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
        db.begin();
        let mut vm_env = OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);

        let values_res = self.run_query_code(&mut vm_env, &main_code_id, code);

        let (mut db, _) = vm_env
            .destruct()
            .expect("Failed to recover database reference after executing transaction");

        db.roll_back()?;
        Ok(values_res?)
    }
}

#[test]
fn test_render_codec() {
    core::init(true, "localhost", 20443);

    let txt = "hello world!";
    let bytes = Renderer::encode_bytes(txt.as_bytes()).unwrap();

    let mut bytes_decoded = vec![];
    Renderer::decode(&mut &bytes[..], &mut bytes_decoded).unwrap();
    let s = std::str::from_utf8(&bytes_decoded).unwrap();

    assert_eq!(&s, &txt);
}

#[test]
fn test_render_viewports_raw_text() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-render-eval-hello-world";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
(wrb-root u60 u80)
(wrb-viewport u0 u5 u5 u25 u25)
(wrb-viewport u1 u20 u20 u25 u25)
(wrb-viewport u2 u35 u35 u25 u25)
(wrb-static-txt-immediate u0 u0 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u1 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u2 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u3 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u4 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u5 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u6 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u7 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u8 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u9 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u10 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u11 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u12 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u13 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u14 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u15 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u16 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u17 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u18 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-static-txt-immediate u0 u19 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")

(wrb-static-txt-immediate u1 u0 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u1 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u2 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u3 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u4 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u5 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u6 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u7 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u8 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u9 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u10 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u11 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u12 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u13 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u14 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u15 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u16 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u17 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u18 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-static-txt-immediate u1 u19 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")

(wrb-static-txt-immediate u2 u0 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u1 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u2 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u3 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u4 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u5 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u6 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u7 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u8 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u9 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u10 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u11 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u12 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u13 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u14 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u15 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u16 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u17 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u18 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-static-txt-immediate u2 u19 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
"#;
    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc", 1).unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_string(&mut vm, &bytes).unwrap();
    println!("{}", &s);

    let mut vm = ClarityVM::new(db_path, "foo-test.btc", 1).unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();
    println!("{}", &s);
}

#[test]
fn test_render_viewports_wrapped_text() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-render-eval-hello-world-wrapped";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
(wrb-root u60 u80)
(wrb-viewport u0 u5 u5 u25 u25)
(wrb-viewport u1 u20 u20 u25 u25)
(wrb-viewport u2 u35 u35 u25 u25)

(wrb-static-print-immediate u0 none u0 (buff-to-uint-be 0x0000ff) u"This is the blue song that never ends.  ")
(wrb-static-print-immediate u1 none u0 (buff-to-uint-be 0x00ff00) u"This is the green song that never ends.  ")
(wrb-static-print-immediate u2 none u0 (buff-to-uint-be 0xff0000) u"This is the red song that never ends.  ")

(wrb-static-println-immediate u0 none u0 (buff-to-uint-be 0x0000ff) u"Yes, it goes on and on, my friends.")
(wrb-static-println-immediate u1 none u0 (buff-to-uint-be 0x00ff00) u"Yes, it goes on and on, my friends.")
(wrb-static-println-immediate u2 none u0 (buff-to-uint-be 0xff0000) u"Yes, it goes on and on, my friends.")

(wrb-static-print-immediate u0 none u0 (buff-to-uint-be 0x0000ff) u"Some people started signing it, not knowing what it was...")
(wrb-static-print-immediate u1 none u0 (buff-to-uint-be 0x00ff00) u"Some people started signing it, not knowing what it was...")
(wrb-static-print-immediate u2 none u0 (buff-to-uint-be 0xff0000) u"Some people started singing it, not knowing what it was...")
"#;

    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc", 1).unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_string(&mut vm, &bytes).unwrap();
    println!("====== output ======\n{}\n====== end output ======", &s);

    let mut vm = ClarityVM::new(db_path, "foo-test.btc", 1).unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();

    assert_eq!(
        s,
        "                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
     This is the blue song                                                      
     that never ends.  Yes,                                                     
     it goes on and on, my                                                      
     friends.                                                                   
     Some people started                                                        
     signing it, not knowing                                                    
     what it was...                                                             
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                    This is the green song                                      
                    that never ends.  Yes,                                      
                    it goes on and on, my                                       
                    friends.                                                    
                    Some people started                                         
                    signing it, not knowing                                     
                    what it was...                                              
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                   This is the red song                         
                                   that never ends.  Yes,                       
                                   it goes on and on, my                        
                                   friends.                                     
                                   Some people started                          
                                   singing it, not knowing                      
                                   what it was...                               
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                "
    );
}

#[test]
fn test_render_viewport_buttons() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-render-viewport-buttons";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
(wrb-root u6 u10)
(wrb-viewport u0 u0 u0 u2 u10)
(wrb-viewport u1 u2 u0 u2 u10)
(wrb-viewport u2 u4 u0 u2 u10)

(define-constant WRB_BUTTON_1 (wrb-button u0 u0 u0 u"button 1"))
(define-constant WRB_BUTTON_2 (wrb-button u1 u0 u5 u"button 2"))
(define-constant WRB_BUTTON_3 (wrb-button u2 u0 u9 u"button 3"))
"#;
    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc", 1).unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_string(&mut vm, &bytes).unwrap();
    println!("{}", &s);

    let mut vm = ClarityVM::new(db_path, "foo-test.btc", 1).unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();
    assert_eq!(
        s,
        "[button 1]\n          \n     [butt\non 2]     \n         [\nbutton 3] "
    );
}

#[test]
fn test_render_viewport_checkbox() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-render-viewport-checkbox";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
(wrb-root u20 u20)
(wrb-viewport u0 u10 u10 u5 u10)
(define-constant WRB_CHECKBOX_1 (wrb-checkbox u0 u0 u0 (list 
    {
        text: u"option 1",
        selected: false,
    }
    {
        text: u"option 2",
        selected: true
    }
    {
        text: u"looooooooooooooooooooooong option 3",
        selected: false
    })))
"#;
    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc", 1).unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_string(&mut vm, &bytes).unwrap();
    println!("{}", &s);

    let mut vm = ClarityVM::new(db_path, "foo-test.btc", 1).unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();

    println!("{:?}", &s);
    assert_eq!(s, "                    \n                    \n                    \n                    \n                    \n                    \n                    \n                    \n                    \n                    \n          [ ] option\n          [*] option\n          [ ] looooo\n          oooooooooo\n          oooooooong\n                    \n                    \n                    \n                    \n                    ");
}

#[test]
fn test_render_load_store_large_strings() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-load-store-large-string-utf8";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
    (wrb-store-large-string-utf8 u0 u"hello world")
    (asserts! (is-eq (wrb-load-large-string-utf8 u0)) (some u"hello world"))
    (asserts! (is-eq (wrb-load-large-string-utf8 u1)) none)
    "#;

    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc", 1).unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();
    println!("text '{}'", &s);
}
