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

const getImageDate = require('./get-image-date');

/**
 * Generate HTML section for a single test case.
 *
 * @param {string} key
 * @param {object} data Metrics data item from shigehachi
 * @returns {string} Generated HTML section
 * @see https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals
 */
module.exports = (key, data) => {
  const dateA = getImageDate(data.A);
  const dateB = getImageDate(data.B);

  return `
    <section id="${key}">
      <header>
        <h3 class="test-case-name">${data.name}</h3>
        <div class="test-difference">${data.normalised.total}%</div>
      </header>
      <div class="test-images">
        <figure class="test-image-a">
          <img src="${data.A}" alt="">
          <figcaption>
            <time datetime="${dateA.toISOString()}">${dateA.toUTCString()}</time>
          </figcaption>
        </figure>
        <figure class="test-image-diff">
          <img src="${data.diff}" alt="">
          <figcaption>
            Difference metric: ${data.metric}
          </figcaption>
        </figure>
        <figure class="test-image-b">
          <img src="${data.B}" alt="">
          <figcaption>
            <time datetime="${dateB.toISOString()}">${dateB.toUTCString()}</time>
          </figcaption>
        </figure>
      </div>
      <div class="test-actions">
        <div class="test-action-store">
          <button type="button" data-key="${key}" data-action="store">Store</button>
        </div>
        <div class="test-action-accept">
          <button type="button" data-key="${key}" data-action="accept">Accept</button>
        </div>
      </div>
    </section>
  `;
};
