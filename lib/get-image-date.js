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

const fs = require('fs');

/**
 * Get the creation time for the given image file.
 * Returns false when something fails, such as file not found.
 *
 * @param {string} filepath Image file path
 * @returns {Date|boolean} Creation time or false
 * @see https://nodejs.org/docs/latest-v10.x/api/fs.html#fs_stats_birthtime
 */
module.exports = (filepath) => {
  try {
    fs.accessSync(filepath);
  }
  catch (error) {
    console.error(`File not found "${filepath}"`);
    console.error(error.message);

    return false;
  }

  const stat = fs.statSync(filepath);

  return stat.birthtime;
};
