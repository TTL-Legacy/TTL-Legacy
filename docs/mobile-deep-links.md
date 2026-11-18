# Mobile Deep Links

This page lists every deep link URI that the TTL-Legacy Android and iOS apps handle, the
configuration that registers each one, and where it is parsed in the source. It is written from the
code, so if the code changes, update this page with it.

Source of truth:

| Platform | Registration | Parsing |
|---|---|---|
| Android | `mobile/android/app/src/main/AndroidManifest.xml` | `mobile/android/app/src/main/java/com/ttllegacy/services/VaultDeepLinkParser.kt`, `mobile/android/app/src/main/java/com/ttllegacy/ui/MainActivity.kt` |
| iOS | host app Info.plist and entitlements (not checked in, see [iOS](#ios)) | `mobile/ios/TTLLegacy/Sources/Services/UniversalLinkRouter.swift` |

For the Android section of the mobile README, see [`mobile/README.md`](../mobile/README.md).

## Summary

| URI form | Android | iOS |
|---|---|---|
| `ttl-legacy://vault/{vault_id}/{action}` | Yes | No |
| `ttllegacy://vault/{vault_id}/{action}` | Yes | Yes |
| `https://ttl-legacy.app/accept?vault_id={vault_id}` | Yes | No |
| `https://ttl-legacy.app/vaults/{vault_id}/{action}` | Parsed, but not declared in the manifest (see [Android](#android)) | No |
| `https://ttl-legacy.app/vaults/{vault_id}/invite` | No | Yes |
| `https://ttl-legacy.app/vaults/{vault_id}/accept?token={token}` | No | Yes |
| `https://ttl-legacy.app/vaults/{vault_id}/preview` | Parsed as the `preview` action, but not declared in the manifest | Yes |

`{vault_id}` is a single path segment, taken as given (for example `abc-123`). No other URI
schemes are handled: for instance there are no `ttl://` URIs, and the apps do not define links for
passkey registration or for vault release.

## Vault actions

The `{action}` segment of a vault link is one of the values below. Matching is exact and case
sensitive, so `CHECK-IN` is rejected.

| Action | Android | iOS |
|---|---|---|
| `check-in` | Yes | Yes (asks for biometric confirmation before checking in) |
| `withdraw` | Yes | Yes (shows "Withdrawal is not yet available in the mobile app.") |
| `view-details` | Yes | Yes |
| `manage-beneficiary` | Yes | Yes (shows "Beneficiary management is not yet available in the mobile app.") |
| `preview` | Recognised by the parser (`VaultDeepLinkAction.PREVIEW`) | Only as `https://ttl-legacy.app/vaults/{vault_id}/preview` |

A link with a missing vault ID, a missing action, an unknown action or extra path segments is not
recognised and the parser returns nothing (`null` on Android, `nil` on iOS).

## Android

`AndroidManifest.xml` declares three `VIEW` intent filters on `MainActivity` (all with the
`DEFAULT` and `BROWSABLE` categories):

| Intent filter | `<data>` | `android:autoVerify` |
|---|---|---|
| Beneficiary acceptance link | `scheme="https"`, `host="ttl-legacy.app"`, `pathPrefix="/accept"` | `true` |
| Legacy custom scheme | `scheme="ttllegacy"`, `host="vault"` | not set |
| App Links scheme | `scheme="ttl-legacy"`, `host="vault"` | `true` |

`MainActivity` uses `launchMode="singleTask"`. It reads `intent.data` in `onCreate` and
`onNewIntent`, so a link also works while the app is already running.

### Vault links

`VaultDeepLinkParser.parse(uri)` accepts, in `ttl-legacy://` and `ttllegacy://` form:

```
ttl-legacy://vault/{vault_id}/{action}
ttllegacy://vault/{vault_id}/{action}
```

The host must be `vault` and the path must have exactly two segments (vault ID, then action). The
manifest comment mentions `ttl-legacy://vault/{vault_id}` without an action, but the parser
requires the action and rejects a link without it.

A recognised link is turned into the in-app route `vault/{vault_id}/{action}` and shown by
`VaultDeepLinkScreen`. The navigation only happens once the user is signed in; until then the link
stays pending.

The app itself creates `ttllegacy://vault/{vault_id}/check-in` links in `NotificationHelper`: when a
notification is shown for a vault, tapping it and its "Check In Now" action both open this link.

Example:

```
ttl-legacy://vault/abc-123/view-details
ttllegacy://vault/abc-123/check-in
```

### Beneficiary acceptance link

```
https://ttl-legacy.app/accept?vault_id={vault_id}
```

`MainActivity` accepts this link only when the scheme is `https`, the host is `ttl-legacy.app` and
the path is exactly `/accept`; the vault ID comes from the `vault_id` query parameter. It opens the
beneficiary acceptance screen (route `accept/{vault_id}`) once the user is signed in. Vault links
take priority: if the URI is also a valid vault link, the vault link is used.

### HTTPS vault links

`VaultDeepLinkParser` also parses

```
https://ttl-legacy.app/vaults/{vault_id}/{action}
```

(host `ttl-legacy.app`, path `vaults`, vault ID, action). This is the form of the share preview
link, `https://ttl-legacy.app/vaults/{vault_id}/preview`, which both apps create in their "share
vault" screens. However, the manifest only registers the `/accept` path prefix for
`https://ttl-legacy.app`, so Android does not route `/vaults/...` links to the app by itself; the
parser only handles them if they reach `MainActivity` another way.

### Digital Asset Links

On the `https://ttl-legacy.app` filter, `android:autoVerify="true"` makes Android check `https://ttl-legacy.app/.well-known/assetlinks.json`
when the app is installed. The file in this repository is `backend/.well-known/assetlinks.json`. It
lists the package `com.ttllegacy` with the placeholder fingerprint
`REPLACE_WITH_PRODUCTION_SHA256_CERT_FINGERPRINT`, which must be replaced with the SHA-256
fingerprint of the release signing certificate before the app is published. To print it:

```bash
keytool -list -v -keystore release.jks -alias your-alias | grep SHA256
```

## iOS

`UniversalLinkRouter.parse(url:)` turns a URL into one of these `DeepLink` values:

| URL | Result |
|---|---|
| `ttllegacy://vault/{vault_id}/{action}` | `vaultAction(vaultID, action)` (the four actions `check-in`, `withdraw`, `view-details`, `manage-beneficiary`) |
| `https://ttl-legacy.app/vaults/{vault_id}/invite` | `vaultInvitation(vaultID)` |
| `https://ttl-legacy.app/vaults/{vault_id}/accept?token={token}` | `beneficiaryAcceptance(vaultID, token)`; a missing `token` becomes an empty string |
| `https://ttl-legacy.app/vaults/{vault_id}/preview` | `vaultPreview(vaultID)` |

Anything else returns `nil`: other custom-scheme hosts, other hosts, and any path that does not
have exactly the three segments above. The `ttl-legacy://` scheme is not handled on iOS.

`TTLLegacyApp` passes every URL through the router, from two entry points:

- `.onOpenURL` for custom-scheme URLs and universal links opened from other apps;
- `.onContinueUserActivity(NSUserActivityTypeBrowsingWeb)` for universal links opened from
  Safari, Messages and similar (it uses `activity.webpageURL`).

The result is stored in `VaultStore.pendingDeepLink` and shown as a sheet by `DeepLinkView`.

### Required configuration

The repository contains the Swift package only; it has no `Info.plist` or entitlements file. A
comment in `mobile/ios/TTLLegacy/Package.swift` states what the host app needs:

- the Associated Domains entitlement `applinks:ttl-legacy.app`, so that
  `https://ttl-legacy.app/...` links open in the app;
- a URL type for the `ttllegacy` scheme in the host app's Info.plist for the custom-scheme links
  (the router handles the scheme, but registering it is not part of this repository).

Universal links also need an `apple-app-site-association` file served by `ttl-legacy.app`; there is
none in this repository.

## Tests

- Android: `VaultDeepLinkParserTest` (unit) and `VaultDeepLinkIntentTest` (instrumented). See the
  [mobile README](../mobile/README.md#testing) for the commands.
- iOS: `UniversalLinkRouterTests` in `mobile/ios/TTLLegacy/Tests`.
