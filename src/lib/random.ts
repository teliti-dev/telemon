const CHARS = "abcdefghjkmnpqrstuvwxyzABCDEFGHJKMNPQRSTUVWXYZ23456789"

/** Password acak 12 karakter tanpa huruf yang mudah tertukar. */
export function randomPassword() {
  const bytes = crypto.getRandomValues(new Uint8Array(12))
  return Array.from(bytes, (b) => CHARS[b % CHARS.length]).join("")
}
