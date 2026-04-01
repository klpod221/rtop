/**
 * Format bytes to readable string (KB, MB, GB, etc)
 */
export function formatBytes(bytes) {
  if (bytes == null || isNaN(bytes)) return '0 B'
  const numBytes = Number(bytes)
  if (numBytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(numBytes) / Math.log(k))
  if (i < 0) return '0 B'
  return parseFloat((numBytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}

/**
 * Format MHz to a sensible number with units, rounding as needed.
 * Also handles if the input is in another state like GHz.
 */
export function formatMHz(mhz) {
  if (mhz == null || isNaN(mhz)) return '0 MHz'
  const val = Number(mhz)
  if (val >= 1000) {
    return (val / 1000).toFixed(2) + ' GHz'
  }
  return Math.round(val) + ' MHz'
}

/**
 * Format general numbers/percentages to 1 decimal place max.
 */
export function formatPercent(num) {
  if (num == null || isNaN(num)) return '0%'
  return Number(num).toFixed(1).replace(/\.0$/, '') + '%'
}

/**
 * Format power (Watts) with 1 decimal place.
 */
export function formatPower(watts) {
  if (watts == null || isNaN(watts)) return '0 W'
  return Number(watts).toFixed(1) + 'W'
}
