
const fs = require('fs');
const path = require('path');

const getStat = (filepath) => {
  try {
    return fs.statSync(filepath);
  }
  catch {
    return false;
  }
};

module.exports = async (fastify, options) => {
  fastify.get('/', (request, reply) => {
    return { hello: 'world' };
  });

  fastify.get('/list', (request, reply) => {
    return [
      'one',
      'two'
    ];
  });

  fastify.get('/image-info/*', (request, reply) => {
    const filepath = path.normalize(request.params['*']);

    // Does it exist under the image storage?
    
    return getStat(filepath);
  });
};
