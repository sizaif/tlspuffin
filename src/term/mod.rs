// Code in this directory is derived from https://github.com/joshrule/term-rewriting-rs/
// and is licensed under:
//
// The MIT License (MIT)
// Copyright (c) 2018--2021
// Maximilian Ammann <max@maxammann.org>, Joshua S. Rule <joshua.s.rule@gmail.com>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.


mod term;
mod signature;
mod atoms;
mod pretty;

pub use self::term::*;
pub use self::signature::*;
pub use self::atoms::*;

#[cfg(test)]
mod tests {
    use crate::term::{Term, Signature};

    #[test]
    fn example() {
        let mut sig = Signature::default();
        let app = sig.new_op(2, Some("senc".to_string()));
        let s = sig.new_op(0, Some("s".to_string()));
        let k = sig.new_op(0, Some("k".to_string()));

        let constructed_term = Term::Application {
            op: app.clone(),
            args: vec![
                Term::Application {
                    op: app.clone(),
                    args: vec![
                        Term::Application {
                            op: app.clone(),
                            args: vec![
                                Term::Application { op: s.clone(), args: vec![] },
                                Term::Application { op: k.clone(), args: vec![] },
                            ]
                        },
                        Term::Application { op: k.clone(), args: vec![] }
                    ]
                },
                Term::Application {
                    op: app.clone(),
                    args: vec![
                        Term::Application {
                            op: app.clone(),
                            args: vec![
                                Term::Application { op: k.clone(), args: vec![] },
                                Term::Application { op: s.clone(), args: vec![] },
                            ]
                        },
                        Term::Application { op: k, args: vec![] }
                    ]
                }
            ]
        };

        println!("{}", constructed_term.pretty());
    }
}