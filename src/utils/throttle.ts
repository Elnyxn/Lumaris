export function throttle<T extends (...args: never[]) => void>(
  fn: T,
  waitMs: number,
): T & { flush: () => void; cancel: () => void } {
  let last = 0;
  let timer: number | null = null;
  let pendingArgs: Parameters<T> | null = null;

  const run = (args: Parameters<T>) => {
    last = Date.now();
    pendingArgs = null;
    fn(...args);
  };

  const wrapped = ((...args: Parameters<T>) => {
    pendingArgs = args;
    const now = Date.now();
    const remain = waitMs - (now - last);
    if (remain <= 0) {
      if (timer !== null) {
        window.clearTimeout(timer);
        timer = null;
      }
      run(args);
    } else if (timer === null) {
      timer = window.setTimeout(() => {
        timer = null;
        if (pendingArgs) run(pendingArgs);
      }, remain);
    }
  }) as T & { flush: () => void; cancel: () => void };

  wrapped.flush = () => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
    if (pendingArgs) run(pendingArgs);
  };

  wrapped.cancel = () => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
    pendingArgs = null;
  };

  return wrapped;
}

export function debounce<T extends (...args: never[]) => void>(
  fn: T,
  waitMs: number,
): T & { flush: () => void; cancel: () => void } {
  let timer: number | null = null;
  let pendingArgs: Parameters<T> | null = null;

  const wrapped = ((...args: Parameters<T>) => {
    pendingArgs = args;
    if (timer !== null) window.clearTimeout(timer);
    timer = window.setTimeout(() => {
      timer = null;
      if (pendingArgs) {
        const a = pendingArgs;
        pendingArgs = null;
        fn(...a);
      }
    }, waitMs);
  }) as T & { flush: () => void; cancel: () => void };

  wrapped.flush = () => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
    if (pendingArgs) {
      const a = pendingArgs;
      pendingArgs = null;
      fn(...a);
    }
  };

  wrapped.cancel = () => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
    pendingArgs = null;
  };

  return wrapped;
}
