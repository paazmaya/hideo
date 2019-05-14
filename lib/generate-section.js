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

// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals
const templateCase =

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
          <img src="${data.A}">
          <figcaption>
            <time datetime="2019-12-24T13:58+03:00">2019-12-24 13:58 EET</time>
          </figcaption>
        </figure>
        <figure class="test-image-diff">
          <img src="${data.D}">
          <figcaption>
            Difference metric: ${data.metric}
          </figcaption>
        </figure>
        <figure class="test-image-b">
          <img src="${data.B}">
          <figcaption>
            <time datetime="2019-12-25T11:53+03:00">2019-12-25 11:53 EET</time>
          </figcaption>
        </figure>
      </div>
      <div class="test-actions">
        <div class="test-action-store">
          <button type="button">Store</button>
        </div>
        <div class="test-action-accept">
          <button type="button">Accept</button>
        </div>
      </div>
    </section>
  `;
};
