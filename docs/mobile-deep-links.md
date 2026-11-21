# Mobile Deep Links

This page lists every deep link URI that the TTL-Legacy Android and iOS apps handle, the
configuration that registers each one, and where it is parsed in the source. It is written from the
code, so if the code changes, update this page with it.

Source of truth:

| Platform | Registration | Parsing |
|---|---|---|
| Android | `mobile/android/app/src/main/AndroidManifest.xml` | `mobile/android/app/src/main/java/com/ttllegacy/services/VaultDeepLinkParser.kt`, `mobile/android/app/src/main/java/com/ttllegacy/ui/MainActivity.kt` |
| iOS | host app Info.plist and entitlements (not checked in, see [iOS](#ios)) | `mobile/ios/TTLLegacy/Sources/Services/UniversalLinkRouter.swift` |

The Android section of the mobile README also describes these links:
[Deep Link URL Format](../mobile/README.md#deep-link-url-format-android-app-links).

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

`{vault_id}` is a single path segment, taken as given (for example `abc-123`). Query strings and
fragments are ignored on vault links (the only parameters read are `vault_id` on the Android
acceptance link and `token` on the iOS acceptance link). No other URI schemes are handled.

### Links requested in issue #2106

The issue lists four `ttl://` URIs. The `ttl` scheme does not exist in this repository (no match in
`AndroidManifest.xml`, the Android and iOS sources or the backend), so none of them is registered.
The real equivalents are:

| Requested URI | Status | Real equivalent |
|---|---|---|
| `ttl://checkin/{vault_id}` | Not registered | `ttl-legacy://vault/{vault_id}/check-in` (Android) or `ttllegacy://vault/{vault_id}/check-in` (Android and iOS) |
| `ttl://vault/{id}` | Not registered | `ttl-legacy://vault/{id}/view-details` or `ttllegacy://vault/{id}/view-details`. A link without an action, `ttl-legacy://vault/{id}`, matches the Android intent filter but the parser rejects it, so the app only opens |
| `ttl://passkey/register` | Not registered, no equivalent | None. Passkey registration is started inside the app (`PasskeyService.register`, relying party `ttl-legacy.app`), not from a link |
| `ttl://release/{id}` | Not registered, no equivalent | None. `released` is a vault status and a push notification type, not a link target. The nearest action, `withdraw`, only shows a "not yet available" message |

## Vault actions

The `{action}` segment of a vault link is one of the values below. Matching is exact and case
sensitive, so `CHECK-IN` is rejected.

| Action | Android | iOS | What the app shows |
|---|---|---|---|
| `check-in` | Yes | Yes | Confirmation screen; checks in after a biometric prompt. If the vault is not in the loaded list, Android shows "Vault not found" and iOS disables the button |
| `withdraw` | Yes | Yes | "Withdrawal is not yet available in the mobile app." |
| `view-details` | Yes | Yes | Vault details, or "Vault not found" |
| `manage-beneficiary` | Yes | Yes | "Beneficiary management is not yet available in the mobile app." |
| `preview` | Parsed (`VaultDeepLinkAction.PREVIEW`), but `VaultDeepLinkScreen` has no case for it | Only as `https://ttl-legacy.app/vaults/{vault_id}/preview` (`vaultPreview`) | n/a |

A link with a missing vault ID, a missing action, an unknown action or extra path segments is not
recognised and the parser returns nothing (`null` on Android, `nil` on iOS). The app then opens
as it normally would, without navigating anywhere.

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

A recognised link is turned into the in-app route `vault/{vaultId}/{action}` and shown by
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
beneficiary acceptance screen (route `accept/{vaultId}`) once the user is signed in. The manifest
uses `pathPrefix="/accept"`, which also matches paths such as `/accept/x`; the app opens for those
but ignores them. `handleIncomingIntent` tries the vault-link parser first and the acceptance link
second; the two forms do not overlap.

### HTTPS vault links

`VaultDeepLinkParser` also parses

```
https://ttl-legacy.app/vaults/{vault_id}/{action}
```

(host `ttl-legacy.app`, path `vaults`, vault ID, action). This is the form of the share preview
link, `https://ttl-legacy.app/vaults/{vault_id}/preview`, which both apps create in their "share
vault" screens. However, the manifest only registers the `/accept` path prefix for
`https://ttl-legacy.app`, so Android does not route `/vaults/...` links to the app by itself; the
parser only handles them if they reach `MainActivity` another way. Routing them would need an
additional `https` filter with `android:pathPrefix="/vaults/"`.

### Digital Asset Links

On the `https://ttl-legacy.app` filter, `android:autoVerify="true"` makes Android check
`https://ttl-legacy.app/.well-known/assetlinks.json` when the app is installed. The file in this
repository is `backend/.well-known/assetlinks.json`. It lists the package `com.ttllegacy` with the
placeholder fingerprint `REPLACE_WITH_PRODUCTION_SHA256_CERT_FINGERPRINT`, which must be replaced
with the SHA-256 fingerprint of the release signing certificate before the app is published. The
mobile README says the backend serves this file, but no route for it was found in the backend
source, so check that it is reachable at that URL. To print the fingerprint:

```bash
keytool -list -v -keystore release.jks -alias your-alias | grep SHA256
```

Notes:

- Android only verifies filters whose scheme is `http` or `https`. `android:autoVerify="true"` on the
  `ttl-legacy://` filter therefore has no effect, so `ttl-legacy://` links are not verified App Links
  (the manifest comment and the mobile README describe them as verified).
- The manifest also has a `<meta-data android:name="asset_statements">` entry that points to
  `@string/asset_statements`. That string resource is not in this repository.

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

For the `ttl-legacy.app` forms the router checks only the host, not the scheme, so
`http://ttl-legacy.app/vaults/{vault_id}/invite` is also parsed. The test
`test_parse_httpURL_ttlLegacyHost_returnsNil` expects `nil` for it, which does not match the code.

`TTLLegacyApp` passes every URL through the router, from two entry points:

- `.onOpenURL` for custom-scheme URLs and universal links opened from other apps;
- `.onContinueUserActivity(NSUserActivityTypeBrowsingWeb)` for universal links opened from
  Safari, Messages and similar (it uses `activity.webpageURL`).

The result is stored in `VaultStore.pendingDeepLink` and shown as a sheet by `DeepLinkView`.

### Required configuration

The repository contains the Swift package only; it has no `Info.plist`, entitlements file or
`apple-app-site-association` file, so the iOS registration is not checked in. What the host app
needs, derived from the router and from `mobile/ios/TTLLegacy/Package.swift` and `mobile/README.md`:

- The Associated Domains entitlement `applinks:ttl-legacy.app`, so that
  `https://ttl-legacy.app/...` links open in the app (named in a comment in `Package.swift`; the
  mobile README also asks to enable the capability).
- A URL type for the `ttllegacy` scheme in the host app's Info.plist (`CFBundleURLTypes` with
  `CFBundleURLSchemes` set to `ttllegacy`) for the custom-scheme links. Nothing in the repository
  states this; it follows from the router handling that scheme.
- An `apple-app-site-association` file at
  `https://ttl-legacy.app/.well-known/apple-app-site-association` (the mobile README asks for it)
  that lists the app and covers the `/vaults/*` paths. An example with placeholder IDs:

```json
{
  "applinks": {
    "details": [
      { "appIDs": ["<TEAM_ID>.<BUNDLE_ID>"], "components": [{ "/": "/vaults/*" }] }
    ]
  }
}
```

## Tests

- Android: `VaultDeepLinkParserTest` (unit) and `VaultDeepLinkIntentTest` (instrumented). See the
  [mobile README](../mobile/README.md#testing) for the commands.
- iOS: `UniversalLinkRouterTests` in `mobile/ios/TTLLegacy/Tests`.
