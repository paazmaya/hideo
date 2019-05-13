#!/usr/bin/env node

/**
 * Hideo
 * https://github.com/paazmaya/hideo
 *
 * Generate report for visual regression testing
 *
 * Copyright (c) Juga Paazmaya <paazmaya@yahoo.com> (https://paazmaya.fi)
 * Licensed under the MIT license
 */

'use strict';

const fs = require('fs'),
  path = require('path');

const optionator = require('optionator');

const hideo = require('../index');

let pkg;

try {
  const packageJson = fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8');
  pkg = JSON.parse(packageJson);
}
catch (error) {
  console.error('Could not read/parse "package.json", quite strange...');
  console.error(error);
  process.exit(1);
}

const optsParser = optionator({
  prepend: `${pkg.name} [options]`,
  append: `Version ${pkg.version}`,
  options: [
    {
      option: 'help',
      alias: 'h',
      type: 'Boolean',
      default: false,
      description: 'Help and usage instructions'
    },
    {
      option: 'version',
      alias: 'V',
      type: 'Boolean',
      default: false,
      description: 'Version number, with verbosity also application name',
      example: '-vV'
    },
    {
      option: 'verbose',
      alias: 'v',
      type: 'Boolean',
      default: false,
      description: 'Verbose output, will print which file is currently being processed'
    },
    {
      option: 'target',
      alias: 'T',
      type: 'String',
      description: 'Target directory for the HTML site'
    },
    {
      option: 'metrics',
      alias: 'M',
      type: 'String',
      description: 'Metrics data file from shigehachi'
    },
    {
      option: 'directory',
      alias: 'D',
      type: 'String',
      default: '.',
      description: 'Directory for which the metrics are relative'
    }
  ]
});

let opts;

try {
  opts = optsParser.parse(process.argv);
}
catch (error) {
  console.error(error.message);
  process.exit(1);
}

if (opts.version) {
  console.log((opts.verbose ?
    pkg.name + ' v' :
    '') + pkg.version);
  process.exit();
}

if (opts.help) {
  console.log(optsParser.generateHelp());
  process.exit();
}

const options = {
  verbose: typeof opts.verbose === 'boolean' ?
    opts.verbose :
    false,
  target: opts.target,
  metrics: opts.metrics,
  directory: opts.directory
};

hideo(options);
