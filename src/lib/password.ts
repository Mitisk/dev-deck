const CHARSET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*-_=+";

// Крепкий случайный пароль заданной длины (кламп 8..64, дефолт 20).
export function generatePassword(len: number): string {
  const n = Math.max(8, Math.min(64, Math.floor(len) || 20));
  const arr = new Uint32Array(n);
  crypto.getRandomValues(arr);
  let out = "";
  for (let i = 0; i < n; i++) out += CHARSET[arr[i] % CHARSET.length];
  return out;
}

// Грубая оценка надёжности 0..4 (длина + разнообразие классов символов).
export function passwordStrength(pw: string): number {
  if (!pw) return 0;
  let score = 0;
  if (pw.length >= 8) score++;
  if (pw.length >= 14) score++;
  let classes = 0;
  if (/[a-z]/.test(pw)) classes++;
  if (/[A-Z]/.test(pw)) classes++;
  if (/[0-9]/.test(pw)) classes++;
  if (/[^A-Za-z0-9]/.test(pw)) classes++;
  if (classes >= 3) score++;
  if (classes >= 4 && pw.length >= 12) score++;
  return Math.min(4, score);
}
