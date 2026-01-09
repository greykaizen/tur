const DEBUG = false;

export const log = (...args) => DEBUG && console.log(...args);
export const warn = (...args) => DEBUG && console.warn(...args);
export const error = (...args) => DEBUG && console.error(...args);
