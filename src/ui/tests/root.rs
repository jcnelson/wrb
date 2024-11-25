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

use std::fs;
use std::collections::HashMap;

use crate::core;
use crate::ui::charbuff::{CharBuff, CharCell, Color};
use crate::ui::scanline::Scanline;
use crate::ui::root::{Root, SceneGraph};
use crate::ui::viewport::Viewport;
use crate::Renderer;
use crate::ClarityVM;

#[test]
fn test_zbuff() {
    let mut red_viewport = Viewport::new(0, 5, 5, 20, 20);
    let mut green_viewport = Viewport::new(1, 20, 20, 20, 20);
    let mut blue_viewport = Viewport::new(2, 35, 35, 20, 20);

    red_viewport.print_to(10, 0, 0, 0x00ff0000.into(), 0x00ffffff.into(), "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr");
    green_viewport.print_to(11, 0, 0, 0x0000ff00.into(), 0x00ffffff.into(), "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg");
    blue_viewport.print_to(12, 0, 0, 0x000000ff.into(), 0x00ffffff.into(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

    let root = Root::new(60, 80, SceneGraph::new(vec![red_viewport, green_viewport, blue_viewport]));
    let zbuff = root.make_zbuff();

    let zbuff_txt = Root::dump_zbuff(&zbuff, 80);
    println!("{}", &zbuff_txt);

    assert_eq!(
        zbuff_txt,
r#"********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000011111111111111111111****************************************
*****00000000000000011111111111111111111****************************************
*****00000000000000011111111111111111111****************************************
*****00000000000000011111111111111111111****************************************
*****00000000000000011111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111122222222222222222222*************************
********************11111111111111122222222222222222222*************************
********************11111111111111122222222222222222222*************************
********************11111111111111122222222222222222222*************************
********************11111111111111122222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************"#);
}

#[test]
fn test_scenegraph_zbuff() {
    let mut red_viewport = Viewport::new(0, 5, 5, 20, 20);
    let mut green_viewport = Viewport::new_child(1, 0, 1, 1, 20, 20);
    let mut blue_viewport = Viewport::new_child(2, 1, 1, 1, 20, 20);

    red_viewport.print_to(10, 0, 0, 0x00ff0000.into(), 0x00ffffff.into(), "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr");
    green_viewport.print_to(11, 0, 0, 0x0000ff00.into(), 0x00ffffff.into(), "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg");
    blue_viewport.print_to(12, 0, 0, 0x000000ff.into(), 0x00ffffff.into(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

    let root = Root::new(60, 80, SceneGraph::new(vec![red_viewport, green_viewport, blue_viewport]));
    let zbuff = root.make_zbuff();

    let zbuff_txt = Root::dump_zbuff(&zbuff, 80);
    println!("{}", &zbuff_txt);

    assert_eq!(
        zbuff_txt,
        r#"********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
*****00000000000000000000*******************************************************
*****011111111111111111111******************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
*****0122222222222222222222*****************************************************
******122222222222222222222*****************************************************
*******22222222222222222222*****************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************"#);
}

#[test]
fn render_root() {
    let mut red_viewport = Viewport::new(0, 5, 5, 20, 20);
    let mut green_viewport = Viewport::new(1, 20, 20, 20, 20);
    let mut blue_viewport = Viewport::new(2, 35, 35, 20, 20);

    red_viewport.print_to(10, 0, 0, 0x00ff0000.into(), 0x00ffffff.into(), "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr");
    green_viewport.print_to(11, 0, 0, 0x0000ff00.into(), 0x00ffffff.into(), "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg");
    blue_viewport.print_to(12, 0, 0, 0x000000ff.into(), 0x00ffffff.into(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

    let mut root = Root::new(60, 80, SceneGraph::new(vec![red_viewport, green_viewport, blue_viewport]));
    
    let buff = root.refresh();
    let scanlines = Scanline::compile(&buff);
    let mut output = "".to_string();
    for sl in scanlines {
        output.push_str(&sl.into_term_code());
    }
    println!("{}", &output);
}

#[test]
fn test_wordwrap() {
    let mut viewport = Viewport::new(0, 5, 5, 8, 10);
    viewport.print(10, 0, 0, 0.into(), 0xffffff.into(), "no wrap");
    viewport.print(11, 1, 0, 0.into(), 0xffffff.into(), "this is going to wrap");
    viewport.print(12, 3, 0, 0.into(), 0xffffff.into(), "biiiiiiiiiig word");
    viewport.print(13, 6, 0, 0.into(), 0xffffff.into(), "wrap   space");

    let mut root = Root::new(20, 25, SceneGraph::new(vec![viewport]));
    let buff = root.refresh();
    let scanlines = Scanline::compile(&buff);
    let mut output = "".to_string();
    for sl in scanlines.clone() {
        output.push_str(&sl.into_term_code());
    }
    println!("{}", &output);
    assert_eq!(scanlines,
		vec![
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"no wrap".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"             ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"this is ".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"            ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"going to ".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"           ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"wrap".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"                ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"biiiiiiiii".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"          ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"ig word".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"             ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"wrap   ".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"             ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"space".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"               ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
            Scanline::ResetColor,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::ResetColor,
		]); 
}

#[test]
fn test_root_focus_order() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-root-focus-order";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }
    
    let code = r#"
(wrb-root u40 u40)
(wrb-viewport u0 u0 u0 u10 u20)
(wrb-viewport u1 u10 u0 u10 u20)
(wrb-viewport u2 u20 u0 u10 u20)
(wrb-viewport u3 u0 u20 u10 u20)

(define-constant WRB_BUTTON_0 (wrb-button u0 u0 u0 u"button 0"))
(define-constant WRB_BUTTON_1 (wrb-button u0 u0 u10 u"button 1"))
(define-constant WRB_BUTTON_2 (wrb-button u1 u0 u0 u"button 2"))
(define-constant WRB_BUTTON_3 (wrb-button u1 u0 u10 u"button 3"))
(define-constant WRB_BUTTON_4 (wrb-button u2 u0 u0 u"button 4"))
(define-constant WRB_BUTTON_5 (wrb-button u2 u0 u10 u"button 5"))
(define-constant WRB_BUTTON_6 (wrb-button u3 u0 u0 u"button 6"))
(define-constant WRB_BUTTON_7 (wrb-button u3 u0 u10 u"button 7"))
"#;
    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let renderer = Renderer::new(1_000_000_000);
    let mut root = renderer.eval_root(&mut vm, &bytes).unwrap();
    root.refresh();

    let qry = r#"
    (begin
        (print WRB_BUTTON_0)
        (print WRB_BUTTON_1)
        (print WRB_BUTTON_2)
        (print WRB_BUTTON_3)
        (print WRB_BUTTON_4)
        (print WRB_BUTTON_5)
        (print WRB_BUTTON_6)
        (print WRB_BUTTON_7))"#;

    let button_ids : Vec<_> = renderer.run_test_query_code(&mut vm, qry)
        .unwrap()
        .into_iter()
        .map(|val| val.expect_result_ok().unwrap().expect_u128().unwrap())
        .collect();

    eprintln!("button_ids = {:?}", &button_ids);
    eprintln!("root focus order = {:?}", &root.focus_order);

    let mut expected_focus_order = HashMap::new();
    expected_focus_order.insert(button_ids[0], button_ids[1]);
    expected_focus_order.insert(button_ids[1], button_ids[6]);
    expected_focus_order.insert(button_ids[6], button_ids[7]);
    expected_focus_order.insert(button_ids[7], button_ids[2]);
    expected_focus_order.insert(button_ids[2], button_ids[3]);
    expected_focus_order.insert(button_ids[3], button_ids[4]);
    expected_focus_order.insert(button_ids[4], button_ids[5]);
    expected_focus_order.insert(button_ids[5], button_ids[0]);

    assert_eq!(expected_focus_order, root.focus_order);
    assert_eq!(root.focus_first, Some(button_ids[0]));

    // all buttons are materialized
    for element_id in button_ids.iter() {
        assert!(root.forms.get(element_id).is_some());
    }
}

