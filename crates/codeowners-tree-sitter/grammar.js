// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

module.exports = grammar({
  name: 'codeowners',
  rules: {
    source_file: $ => repeat(
      choice(
        $.rule,
        $.comment,
        $.line_without_ending,
        $.line_with_ending
      )
    ),
    rule: $ => seq(
      $.path,
      repeat(seq(
        /\s+/,
        $.owner
      )),
      optional('\n')
    ),
    path: $ => choice(
      $._normal_path,
      $._quoted_path
    ),
    _normal_path: $ => /[^\s#"]+/,
    _quoted_path: $ => seq(
      '"',
      repeat(choice(
        /[^"\\]/,
        seq('\\', /./),
      )),
      '"'
    ),

    owner: $ => choice(
      $.user_owner,
      $.email,
      $.group
    ),
    user_owner: $ => seq(
      '@',
      $.user_identifier
    ),
    user_identifier: $ => /[a-zA-Z0-9_-]+(?:\/[a-zA-Z0-9_-]+)?/,
    email: $ => /[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/,
    group: $ => seq(
      '[',
      $.group_name,
      ']'
    ),
    group_name: $ => /[^\]]+/,
    comment: $ => seq(
      '#',
      /[^\n]*/,
      optional('\n')
    ),
    line_without_ending: $ => seq(
      /[^\s\n#@\["][^\n#]*/
    ),
    line_with_ending: $ => seq(
      /[^\s\n#@\["][^\n#]*/,
      '\n'
    ),
    strategy: $ => choice(
      'least_busy',
      'random',
      'all'
    ),
    user_count: $ => /\d+/,
    number: $ => /\d+/
  },
  extras: $ => []
});
