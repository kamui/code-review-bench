// @flow
// The annotation exactly as the head writes it.
const asWritten: ?Error & { +extensions: mixed } = null;
// The grouping the review says was intended.
const grouped: ?(Error & { +extensions: mixed }) = null;
// The grouping the review says Flow uses.
const tighter: (?Error) & { +extensions: mixed } = null;

function GraphQLErrorLike(originalError?: ?Error & { +extensions: mixed }) {}
GraphQLErrorLike(null);
GraphQLErrorLike(undefined);
