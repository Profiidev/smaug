/** Reactive stand-in for `$app/state`'s `page`, for components deriving from it. */
export const page: { params: Record<string, string>; url: URL } = $state({
  params: {},
  url: new URL('http://localhost/')
});
