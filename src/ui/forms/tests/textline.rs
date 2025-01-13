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

use crate::ui::forms::TextLine;
use crate::ui::forms::WrbForm;
use crate::ui::forms::WrbFormEvent;

use crate::ui::Root;
use crate::ui::SceneGraph;

use termion::event::Key;

#[test]
fn test_textline_handle_event() {
    let mut root = Root::null();
    let mut textline = TextLine::new_detached("".to_string(), 20);
    assert_eq!(textline.text(), "");
    assert_eq!(textline.cursor(), 0);
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char('h')))
        .unwrap();
    assert_eq!(textline.text(), "h");
    assert_eq!(textline.cursor(), 1);
    assert_eq!(textline.insert(), true);

    let mut textline = TextLine::new_detached("hello world".to_string(), 20);

    assert_eq!(textline.text(), "hello world");
    assert_eq!(textline.cursor(), 0);
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Right))
        .unwrap();
    assert_eq!(textline.text(), "hello world");
    assert_eq!(textline.cursor(), 1);
    assert_eq!(textline.insert(), true);

    for _ in 0..100 {
        textline
            .handle_event(&mut root, WrbFormEvent::Keypress(Key::Right))
            .unwrap();
    }
    assert_eq!(textline.text(), "hello world");
    assert_eq!(textline.cursor(), textline.text().len());
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char('!')))
        .unwrap();
    assert_eq!(textline.text(), "hello world!");
    assert_eq!(textline.cursor(), textline.text().len());
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
        .unwrap();
    assert_eq!(textline.text(), "hello world!");
    assert_eq!(textline.cursor(), textline.text().len() - 1);
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
        .unwrap();
    assert_eq!(textline.text(), "hello world!");
    assert_eq!(textline.cursor(), textline.text().len() - 2);
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char('f')))
        .unwrap();
    assert_eq!(textline.text(), "hello worlfd!");
    assert_eq!(textline.cursor(), textline.text().len() - 2);
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Home))
        .unwrap();
    assert_eq!(textline.text(), "hello worlfd!");
    assert_eq!(textline.cursor(), 0);
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::End))
        .unwrap();
    assert_eq!(textline.text(), "hello worlfd!");
    assert_eq!(textline.cursor(), textline.text().len());
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Backspace))
        .unwrap();
    assert_eq!(textline.text(), "hello worlfd");
    assert_eq!(textline.cursor(), textline.text().len());
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
        .unwrap();
    assert_eq!(textline.text(), "hello worlfd");
    assert_eq!(textline.cursor(), textline.text().len() - 1);
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
        .unwrap();
    assert_eq!(textline.text(), "hello worlfd");
    assert_eq!(textline.cursor(), textline.text().len() - 2);
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Delete))
        .unwrap();
    assert_eq!(textline.text(), "hello world");
    assert_eq!(textline.cursor(), textline.text().len() - 1);
    assert_eq!(textline.insert(), true);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Insert))
        .unwrap();
    assert_eq!(textline.text(), "hello world");
    assert_eq!(textline.cursor(), textline.text().len() - 1);
    assert_eq!(textline.insert(), false);

    textline
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char(' ')))
        .unwrap();
    assert_eq!(textline.text(), "hello worl ");
    assert_eq!(textline.cursor(), textline.text().len());
    assert_eq!(textline.insert(), false);
}
