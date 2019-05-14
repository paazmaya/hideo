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

const tape = require('tape');
const getImageDate = require('../../lib/get-image-date');

tape('getImageDate - finds date from LICENSE', (test) => {
  test.plan(1);

  const date = getImageDate('LICENSE');

  test.equal(date.getFullYear(), 2019);
});

tape('getImageDate - not found file', (test) => {
  test.plan(1);

  const date = getImageDate('not-here-you-know');

  test.notOk(date);
});
