import '@testing-library/jest-dom'

// Suppress jsdom warning for HTML5 <search> element (not yet recognized by jsdom)
const originalError = console.error
console.error = (...args) => {
  if (typeof args[0] === 'string' && args[0].includes('unrecognized in this browser')) return
  originalError(...args)
}
