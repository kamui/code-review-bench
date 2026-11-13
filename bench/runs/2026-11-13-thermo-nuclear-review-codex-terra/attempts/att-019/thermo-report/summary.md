[P3] Restore the stackless original error fixture — `src/error/__tests__/GraphQLError-test.js:58`

The replacement `new Error('original')` already has a stack in Node, so this test takes the `originalError.stack` branch in `GraphQLError` and duplicates the preceding test instead of exercising the fallback that creates a stack when the original error has none. A regression that leaves stackless original errors without a stack would now pass this test. Use a Flow-compatible error fixture whose stack is absent and assert that the resulting stack was generated.
