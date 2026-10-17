# Third-party notices

`src/gherkin-keywords.json` extracts the `feature`, `rule`, and
`scenarioOutline` entries from `src/languages.json` in
[cucumber-rs/gherkin 0.14.0](https://crates.io/crates/gherkin/0.14.0).
Keep these entries aligned with the pinned parser when updating it. They
support AST cardinality/outline checks; Gherkin syntax is parsed by the crate.
The extracted data is used under the upstream MIT license:

Copyright (c) 2018-2023  Brendan Molloy <brendan@bbqsrc.net>

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.


## ADR template

`src/templates/adr.md` is a BADR-owned adaptation of
[adr/madr 4.0.0, template/adr-template.md](https://github.com/adr/madr/blob/4.0.0/template/adr-template.md),
following the reference adaptation in kata at commit
`faaae5d97386941810bb1a7ac319aafd753c95fd`. It retains the required reasoning
sections and makes unresolved proposal content explicit. BADR supplies its own
identity/status/date frontmatter and does not depend on kata at runtime.
Optional Confirmation and detailed option analysis may be added by authors.

Upstream offers MIT OR CC0-1.0. The chosen license for this template adaptation
is **CC0-1.0**, retained from the reference adaptation. SPDX-License-Identifier:
CC0-1.0. This adaptation is dedicated to the public domain under
[CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/).
