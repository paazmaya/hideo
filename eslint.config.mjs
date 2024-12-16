import paazmaya from 'eslint-config-paazmaya';

export default [
  paazmaya,
  {
    parserOptions: {
      ecmaVersion: 'latest'
    },
    rules: {
      'no-console': 0,
      'no-process-exit': 0
    }
  }
];