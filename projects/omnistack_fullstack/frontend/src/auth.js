// Client Auth Subsystem
export const AuthManager = {
  tokenKey: "omnistack_token",
  getToken() { return localStorage.getItem(this.tokenKey); },
  setToken(t) { localStorage.setItem(this.tokenKey, t); },
  clear() { localStorage.removeItem(this.tokenKey); },
  isAuthenticated() { return !!this.getToken(); }
};
