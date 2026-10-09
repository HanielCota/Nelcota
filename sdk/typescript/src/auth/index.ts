/** `@nelcota/client/auth`: accounts, sessions and sign-in. */

export { AuthClient } from './client.js';
export type {
  AuthEvent,
  AuthListener,
  AuthOptions,
  Credentials,
  EmailLink,
  EmailLinkType,
  OAuthInput,
  OAuthProvider,
  SignUpInput,
} from './client.js';
export { memoryStorage } from './session.js';
export type { Session, SessionStorage, User } from './session.js';
