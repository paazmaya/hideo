# Hideo (秀雄)

> Visual regression testing reporter

![Mrs Hideo Sonobe](./logo.png)

[![Build Status](https://semaphoreci.com/api/v1/paazmaya/hideo/branches/master/shields_badge.svg)](https://semaphoreci.com/paazmaya/hideo)

This tool generates an report, one or more web pages, from directories of screen captures and a possible meta data, such as generated by `shigehachi`

## Background for the name

The name of the project is for honouring the legacy of Mrs Sonobe Hideo (園部 秀雄),
who was the 15th head master of
[Jikishinkageryu Naginatajutsu (直心影流薙刀術)](https://naginata.fi/en/koryu),
which is an ancient Japanese martial art, focusing the handling of a long pole like weapon
called naginata.

## Getting started

Please note that the minimum supported version of [Node.js](https://nodejs.org/en/) is `8.11.1`, which is [the active Long Term Support (LTS) version](https://github.com/nodejs/Release#release-schedule).

The metrics input is expected to look something similar to, such as provided by shigehachi version 6 onwards:

```json
{
  "16bf19e046e752c8a14df3799befa102d8a30a0e": {
    "name": "Running PostCSS",
    "A": "tests/fixtures/prev/postcss.png",
    "B": "tests/fixtures/curr/postcss.png",
    "diff": "tests/fixtures/diff/postcss-composite.png",
    "metric": "PeakAbsoluteError",
    "normalised": {
      "red": "0.1411764706",
      "green": "0.7882352941",
      "blue": "0.7960784314",
      "opacity": "1.0000000000",
      "total": "1.0000000000"
    }
  }
}
```

The resulting HTML site is build by using JavaScript [template strings](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals).

## Command line options

The output of `hideo --help` pretty much covers all the options:

```sh
```

## Contributing

["A Beginner's Guide to Open Source: The Best Advice for Making your First Contribution"](http://www.erikaheidi.com/blog/a-beginners-guide-to-open-source-the-best-advice-for-making-your-first-contribution/).

[Also there is a blog post about "45 Github Issues Dos and Don’ts"](https://davidwalsh.name/45-github-issues-dos-donts).

Linting is done with [ESLint](http://eslint.org) and can be executed with `npm run lint`.
There should be no errors appearing after any JavaScript file changes.

Unit tests are written with [`tape`](https://github.com/substack/tape) and can be executed with `npm test`.
Code coverage is inspected with [`nyc`](https://github.com/istanbuljs/nyc) and
can be executed with `npm run coverage` after running `npm test`.
Please make sure it is over 90% at all times.

## Version history

* `v0.1.0` (2019-05-)
  - Use [`npm-shrinkwrap.json`](https://docs.npmjs.com/files/shrinkwrap.json) for locking the working set of 3rd party dependencies

## License

Copyright (c) [Juga Paazmaya](https://paazmaya.fi) <paazmaya@yahoo.com>

Licensed under [the MIT license](./LICENSE).
