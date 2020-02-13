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

const generatePage = require('./lib/generate-page');

/**
 * @param {object}  options           Set of options
 * @param {boolean} options.verbose   Set the output more verbose
 * @param {string}  options.target    Target directory for the HTML site
 * @param {object}  options.metrics   Metrics data from shigehachi
 * @param {string}  options.directory Directory for which the metrics are relative
 *
 * @returns {void}
 */
module.exports = (options) => {
  const html = generatePage(options.metrics, options.directory);
  fs.writeFileSync(path.join(options.target, 'hideo.html'), html, 'utf8');
};
