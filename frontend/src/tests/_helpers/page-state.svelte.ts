/** Reactive stand-in for `$app/state`'s `page`, for components deriving from it. */
export const page: { params: Record<string, string>; url: URL } = $state({
  params: {},
  url: new URL('http://localhost/')
});

// Superforms 3 also reads these from `$app/state`.
export const navigating = {
  complete: Promise.resolve(),
  from: null,
  to: null,
  type: null
};
export const updated = { current: false };
