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

use crate::ui::forms::textarea::GapBuffer;
use crate::ui::forms::textarea::GapBufferIterator;
use crate::ui::forms::TextArea;
use crate::ui::forms::WrbForm;
use crate::ui::forms::WrbFormEvent;

use crate::ui::Root;
use crate::ui::SceneGraph;

use termion::event::Key;

#[test]
fn test_gapbuffer_insert_backspace_delete_replace_left_right_ops() {
    let mut gb = GapBuffer::new("", 10);
    assert_eq!(gb.len(), 0);
    assert_eq!(gb.gap, 10);

    gb.insert('h');
    gb.insert('e');
    gb.insert('l');
    gb.insert('l');
    gb.insert('o');
    gb.insert(' ');

    assert_eq!(gb.len(), 6);
    assert_eq!(gb.gap, 4);
    assert_eq!(gb.cursor, 6);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec!['h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, 'o' as u32, ' ' as u32, 0, 0, 0, 0]
    );
    assert_eq!(gb.to_string(usize::MAX), "hello ");

    gb.left();
    gb.left();

    assert_eq!(gb.len(), 6);
    assert_eq!(gb.gap, 4);
    assert_eq!(gb.cursor, 4);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec!['h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, 0, 0, 0, 0, 'o' as u32, ' ' as u32]
    );
    assert_eq!(gb.to_string(usize::MAX), "hello ");

    gb.insert(' ');
    gb.insert('n');

    assert_eq!(gb.len(), 8);
    assert_eq!(gb.gap, 2);
    assert_eq!(gb.cursor, 6);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 0, 0,
            'o' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell no ");

    gb.right();
    gb.right();

    assert_eq!(gb.len(), 8);
    assert_eq!(gb.gap, 2);
    assert_eq!(gb.cursor, 8);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            ' ' as u32, 0, 0
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell no ");

    gb.left();

    assert_eq!(gb.len(), 8);
    assert_eq!(gb.gap, 2);
    assert_eq!(gb.cursor, 7);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32, 0,
            0, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell no ");

    gb.insert('p');
    gb.insert('e');

    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 0);
    assert_eq!(gb.cursor, 9);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope ");

    // left/right on a full buffer
    gb.left();
    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 0);
    assert_eq!(gb.cursor, 8);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope ");

    gb.left();
    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 0);
    assert_eq!(gb.cursor, 7);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope ");

    gb.right();
    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 0);
    assert_eq!(gb.cursor, 8);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope ");

    gb.right();
    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 0);
    assert_eq!(gb.cursor, 9);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope ");

    // right overflow
    gb.right();
    gb.right();
    gb.right();
    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 0);
    assert_eq!(gb.cursor, 10);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope ");

    // left overflow
    for _ in 0..11 {
        gb.left();
    }

    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 0);
    assert_eq!(gb.cursor, 0);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope ");

    for _ in 0..11 {
        gb.right();
    }

    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 0);
    assert_eq!(gb.cursor, 10);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope ");

    gb.left();
    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 0);
    assert_eq!(gb.cursor, 9);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope ");

    // realloc
    gb.insert('!');
    assert_eq!(gb.len(), 11);
    assert_eq!(gb.gap, 9);
    assert_eq!(gb.cursor, 10);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, '!' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, ' ' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope! ");

    gb.right();
    assert_eq!(gb.len(), 11);
    assert_eq!(gb.gap, 9);
    assert_eq!(gb.cursor, 11);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, '!' as u32, ' ' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope! ");

    gb.backspace();
    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 10);
    assert_eq!(gb.cursor, 10);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, ' ' as u32, 'n' as u32, 'o' as u32,
            'p' as u32, 'e' as u32, '!' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope!");

    gb.left();
    gb.left();
    gb.left();
    gb.left();
    gb.left();
    gb.left();

    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 10);
    assert_eq!(gb.cursor, 4);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ' ' as u32, 'n' as u32, 'o' as u32, 'p' as u32, 'e' as u32, '!' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hell nope!");

    gb.backspace();
    assert_eq!(gb.len(), 9);
    assert_eq!(gb.gap, 11);
    assert_eq!(gb.cursor, 3);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ' ' as u32,
            'n' as u32, 'o' as u32, 'p' as u32, 'e' as u32, '!' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "hel nope!");

    gb.insert('p');
    assert_eq!(gb.len(), 10);
    assert_eq!(gb.gap, 10);
    assert_eq!(gb.cursor, 4);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'p' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ' ' as u32, 'n' as u32, 'o' as u32, 'p' as u32, 'e' as u32, '!' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "help nope!");

    gb.delete();
    assert_eq!(gb.len(), 9);
    assert_eq!(gb.gap, 11);
    assert_eq!(gb.cursor, 4);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'p' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            'n' as u32, 'o' as u32, 'p' as u32, 'e' as u32, '!' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "helpnope!");

    gb.delete();
    assert_eq!(gb.len(), 8);
    assert_eq!(gb.gap, 12);
    assert_eq!(gb.cursor, 4);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'p' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            'o' as u32, 'p' as u32, 'e' as u32, '!' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "helpope!");

    gb.right();
    gb.right();
    gb.right();
    gb.right();

    gb.delete();
    assert_eq!(gb.len(), 7);
    assert_eq!(gb.gap, 13);
    assert_eq!(gb.cursor, 7);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'p' as u32, 'o' as u32, 'p' as u32, 'e' as u32, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "helpope");

    gb.left();
    gb.left();
    gb.left();
    gb.left();

    assert_eq!(gb.len(), 7);
    assert_eq!(gb.gap, 13);
    assert_eq!(gb.cursor, 3);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 'p' as u32,
            'o' as u32, 'p' as u32, 'e' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "helpope");

    gb.replace('i');
    assert_eq!(gb.len(), 7);
    assert_eq!(gb.gap, 13);
    assert_eq!(gb.cursor, 3);
    assert_eq!(gb.line_start, 0);
    assert_eq!(
        gb.buffer,
        vec![
            'h' as u32, 'e' as u32, 'l' as u32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 'i' as u32,
            'o' as u32, 'p' as u32, 'e' as u32
        ]
    );
    assert_eq!(gb.to_string(usize::MAX), "heliope");
}

#[test]
fn test_gapbuffer_realloc() {
    let mut gb = GapBuffer::new("", 10);
    assert_eq!(gb.len(), 0);
    assert_eq!(gb.gap, 10);

    let text = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    // insert at end
    for c in text.chars() {
        gb.insert(c);
    }
    assert_eq!(gb.to_string(usize::MAX), text);

    // insert at middle
    let mut gb = GapBuffer::new("", 10);
    assert_eq!(gb.len(), 0);
    assert_eq!(gb.gap, 10);

    for (i, c) in text.chars().enumerate() {
        if i > 0 {
            for _ in 0..((i - 1) / 2) {
                gb.left();
            }
        }
        gb.insert(c);
    }
}

#[test]
fn test_gapbuffer_line_start_end() {
    let test_vectors = vec![
        "",
        "\n",
        "This is the first line.",
        "This is the first line.\n",
        "This is the first line.\n\n\n\n\n",
        "This is the first line.\n\n\n\n\nThis is the sixth line.",
        "This is the first line.\n\n\n\n\nThis is the sixth line.\n",
        "This is the first line.\nThis is the second line.\nThis is the third line.\nThis is the fourth line.\nThis is the fifth line.\nThis is the sixth line.\nThis is the seventh line.\nThis is the eight line.\nThis is the ninth line.\nThis is the tenth line.",
        "This is the first line.\nThis is the second line.\nThis is the third line.\nThis is the fourth line.\nThis is the fifth line.\nThis is the sixth line.\nThis is the seventh line.\nThis is the eight line.\nThis is the ninth line.\nThis is the tenth line.\n",
        "\n\n\n\n\n\n\n\n\n\n",
    ];

    for initial_text in test_vectors.into_iter() {
        let mut gb = GapBuffer::new("", 10);
        assert_eq!(gb.len(), 0);
        assert_eq!(gb.gap, 10);

        eprintln!("test vector: '{}'", &initial_text);

        let line_offsets = {
            let mut offsets = vec![0];
            for (i, chr) in initial_text.chars().enumerate() {
                if chr == '\n' {
                    offsets.push(i + 1);
                }
            }
            offsets
        };

        eprintln!("line offsets: {:?}", &line_offsets);

        for chr in initial_text.chars() {
            gb.insert(chr);
        }

        while gb.cursor != 0 {
            gb.left();
        }
        assert_eq!(gb.line_start, 0);
        assert_eq!(gb.cursor, 0);

        let mut line_idx = 0;

        // line_start updates with right()
        while gb.cursor < initial_text.len() {
            if line_idx < line_offsets.len() {
                if gb.cursor == line_offsets[line_idx] {
                    assert_eq!(
                        gb.line_start, line_offsets[line_idx],
                        "line_idx = {}",
                        line_idx
                    );
                    line_idx += 1;
                }
            } else {
                assert!(gb.chr().unwrap() != '\n');
            }
            gb.right();
        }
        if initial_text.len() > 0 {
            assert!(line_idx + 1 >= line_offsets.len());
        } else {
            assert_eq!(line_idx, 0);
        }
        if line_idx >= line_offsets.len() {
            line_idx = line_offsets.len() - 1
        }
        assert_eq!(gb.cursor, initial_text.len());
        assert_eq!(
            gb.line_start, line_offsets[line_idx],
            "line_idx = {}",
            line_idx
        );

        // line_start updates with left()
        let mut done = false;
        while gb.cursor > 0 {
            if !done {
                if gb.cursor == line_offsets[line_idx] {
                    assert_eq!(
                        gb.line_start, line_offsets[line_idx],
                        "line_idx = {}",
                        line_idx
                    );
                    if line_idx > 0 {
                        line_idx -= 1;
                    } else {
                        done = true;
                    }
                } else if gb.cursor + 1 == line_offsets[line_idx] {
                    assert!(gb.chr().is_none() || gb.chr().unwrap() == '\n');
                }
            } else {
                if gb.cursor + gb.gap < gb.buffer.len() {
                    assert!(gb.chr().is_none() || gb.chr().unwrap() != '\n');
                }
            }
            gb.left();
        }
        assert_eq!(line_idx, 0);
        assert_eq!(gb.cursor, 0);

        for i in 0..(line_offsets.len() - 1) {
            gb.line_end();

            // we're at the \n
            assert_eq!(gb.cursor + 1, line_offsets[i + 1]);

            gb.line_start();
            assert_eq!(gb.cursor, line_offsets[i]);
            assert_eq!(gb.cursor, gb.line_start);

            gb.line_end();
            gb.right();
        }
    }
}

#[test]
fn test_gapbuffer_up_down() {
    // 5 row window, 10 column window
    let mut gb = GapBuffer::new("", 10);
    assert_eq!(gb.len(), 0);
    assert_eq!(gb.gap, 10);

    let initial_text = "This is the first line.\nThis is the second line.\nThis is the third line.\nThis is the fourth line.\nThis is the fifth line.\nThis is the sixth line.\nThis is the seventh line.\nThis is the eighth line.\nThis is the ninth line.\nThis is the tenth line.";
    let line_offsets = {
        let mut offsets = vec![0];
        for (i, chr) in initial_text.chars().enumerate() {
            if chr == '\n' {
                offsets.push(i + 1);
            }
        }
        offsets
    };
    eprintln!("initial text\n{}", &initial_text);
    eprintln!("offsets = {:?}", &line_offsets);

    for chr in initial_text.chars() {
        gb.insert(chr);
    }

    // check line-start
    while gb.cursor != 0 {
        gb.left();
    }

    let mut line_idx = 0;
    while gb.cursor < initial_text.len() {
        if line_idx < line_offsets.len() {
            if gb.cursor == line_offsets[line_idx] {
                assert_eq!(gb.line_start, line_offsets[line_idx]);
                line_idx += 1;
            }
        } else {
            assert!(gb.chr().unwrap() != '\n');
        }
        gb.right();
    }

    // reset
    while gb.cursor > 0 {
        gb.left();
    }
    assert_eq!(gb.line_start, 0);
    assert_eq!(gb.cursor, 0);

    // line_start() / line_end();
    for i in 0..(line_offsets.len() - 1) {
        gb.line_end();

        // we're at the \n
        assert_eq!(gb.cursor + 1, line_offsets[i + 1]);

        gb.line_start();
        assert_eq!(gb.cursor, line_offsets[i]);
        assert_eq!(gb.cursor, gb.line_start);

        gb.line_end();
        gb.right();
    }

    // reset
    while gb.cursor > 0 {
        gb.left();
    }
    assert_eq!(gb.line_start, 0);
    assert_eq!(gb.cursor, 0);

    // go down the lines
    for i in 1..line_offsets.len() {
        let scroll_col = 0;
        gb.down(scroll_col);
        assert_eq!(gb.cursor, gb.line_start);
        assert_eq!(gb.cursor, line_offsets[i]);
    }

    // go back up the lines
    for i in (0..(line_offsets.len() - 1)).rev() {
        eprintln!("cursor is {}, line_start is {}", gb.cursor, gb.line_start);
        let scroll_col = 0;
        gb.up(scroll_col);
        assert_eq!(gb.cursor, gb.line_start);
        assert_eq!(gb.cursor, line_offsets[i]);
    }
}

#[test]
fn test_textarea_handle_event() {
    let mut root = Root::null();
    let mut textarea = TextArea::new_detached("".to_string(), 5, 20, 2_000);
    assert_eq!(textarea.text(usize::MAX), "");
    assert_eq!(textarea.cursor(), 0);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char('h')))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "h");
    assert_eq!(textarea.cursor(), 1);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    let mut textarea = TextArea::new_detached("hello world".to_string(), 5, 20, 2_000);

    assert_eq!(textarea.text(usize::MAX), "hello world");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello world");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 1);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    for _ in 0..100 {
        textarea
            .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
            .unwrap();
    }
    assert_eq!(textarea.text(usize::MAX), "hello world");
    assert_eq!(textarea.cursor(), 0);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::End))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello world");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Home))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello world");
    assert_eq!(textarea.cursor(), 0);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::End))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello world");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char('!')))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello world!");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello world!");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 1);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello world!");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 2);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char('f')))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello worlfd!");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 2);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Home))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello worlfd!");
    assert_eq!(textarea.cursor(), 0);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::End))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello worlfd!");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Backspace))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello worlfd");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello worlfd");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 1);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello worlfd");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 2);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Delete))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello world");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 1);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Insert))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello world");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 1);
    assert_eq!(textarea.insert(), false);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char(' ')))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello worl ");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), false);
    assert_eq!(textarea.scroll(), 0);

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Insert))
        .unwrap();
    assert_eq!(textarea.text(usize::MAX), "hello worl ");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    for _i in 0..5 {
        textarea
            .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char('\n')))
            .unwrap();
        assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
        assert_eq!(textarea.insert(), true);

        textarea
            .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char('a')))
            .unwrap();
        assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
        assert_eq!(textarea.insert(), true);
    }
    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Char('b')))
        .unwrap();
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), true);

    assert_eq!(textarea.text(usize::MAX), "hello worl \na\na\na\na\nab");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len());
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 12);

    for _i in 0..11 {
        textarea
            .handle_event(&mut root, WrbFormEvent::Keypress(Key::Left))
            .unwrap();
    }

    assert_eq!(textarea.text(usize::MAX), "hello worl \na\na\na\na\nab");
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 11);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 0);

    for _i in 0..8 {
        textarea
            .handle_event(&mut root, WrbFormEvent::Keypress(Key::Right))
            .unwrap();
        assert_eq!(textarea.text(usize::MAX), "hello worl \na\na\na\na\nab");
        assert_eq!(textarea.insert(), true);
        assert_eq!(textarea.scroll(), 0);
    }

    textarea
        .handle_event(&mut root, WrbFormEvent::Keypress(Key::Right))
        .unwrap();
    assert_eq!(textarea.cursor(), textarea.text(usize::MAX).len() - 2);
    assert_eq!(textarea.insert(), true);
    assert_eq!(textarea.scroll(), 12);
}

#[test]
fn test_textarea_gap_buffer_iter() {
    let mut gb = GapBuffer::new("", 10);
    let initial_text = "This is the first line.\nThis is the second line.\nThis is the third line.\nThis is the fourth line.\nThis is the fifth line.\nThis is the sixth line.\nThis is the seventh line.\nThis is the eighth line.\nThis is the ninth line.\nThis is the tenth line.";
    for chr in initial_text.chars() {
        gb.insert(chr);
    }

    eprintln!("gap buffer:\n{}", gb.to_string(30));
}
