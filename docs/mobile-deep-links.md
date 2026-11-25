# Mobile Deep Links

This document enumerates every deep link / universal link URI scheme handled by the TTL mobile apps (iOS and Android). For each URI it describes the parameters, a concrete example, the expected app behavior, and the platform configuration required to register it.

> **Note on scheme naming:** The original tracking issue (#2106) refers to `ttl://` schemes (`ttl://checkin/{vault_id}`, `ttl://vault/{id}`, `ttl://passkey/register`, `ttl://release/{id}`). The **actually implemented** schemes use the `ttllegacy://` and `ttl-legacy://` prefixes plus HTTPS App Links / Universal Links on the `ttl-legacy.app` domain. This document describes what is implemented. See [Discrepancy with the issue](#discrepancy-with-the-issue) for details.

For the passkey registration flow referenced by some of these links, see [docs/mobile-passkey-flow.md](./mobile-passkey-flow.md).

---

## Overview of supported URIs

### Android

| Scheme / Host | Path | Purpose |
| --- | --- | --- |
| `ttllegacy://` | `/vault/{vault_id}/{action}` | Custom scheme deep link to a vault action |
| `ttl-legacy://` | `/vault/{vault_id}/{action}` | Alternate custom scheme deep link to a vault action |
| `https://ttl-legacy.app` | `/vaults/{vault_id}/{action}` | HTTPS App Link to a vault action |
| `https://ttl-legacy.app` | `/accept?vault_id=...` | Beneficiary acceptance App Link |

Supported `{action}` values on Android: `check-in`, `withdraw`, `view-details`, `manage-beneficiary`, `preview`.

### iOS

| Scheme / Host | Path | Purpose |
| --- | --- | --- |
| `https://ttl-legacy.app` | `/vaults/{vault_id}/invite` | Universal Link to a vault invite |
| `https://ttl-legacy.app` | `/vaults/{vault_id}/accept?token=...` | Universal Link for beneficiary acceptance |
| `https://ttl-legacy.app` | `/vaults/{vault_id}/preview` | Universal Link to a vault preview |
| `ttllegacy://` | `/vault/{vault_id}/{action}` | Custom scheme deep link to a vault action |

---

## Android

### Custom schemes: `ttllegacy://` and `ttl-legacy://`

Both custom schemes share the same path structure:

```
ttllegacy://vault/{vault_id}/{action}
ttl-legacy://vault/{vault_id}/{action}
```

**Parameters**

| Parameter | Location | Description |
| --- | --- | --- |
| `vault_id` | path segment | The vault identifier (string / UUID). |
| `action` | path segment | One of `check-in`, `withdraw`, `view-details`, `manage-beneficiary`, `preview`. |

**Examples**

```
ttllegacy://vault/abc123/check-in
ttllegacy://vault/abc123/withdraw
ttllegacy://vault/abc123/view-details
ttllegacy://vault/abc123/manage-beneficiary
ttllegacy://vault/abc123/preview
ttl-legacy://vault/abc123/check-in
```

**Expected app behavior**

The app parses the URI, extracts `vault_id` and `action`, and routes to the corresponding vault screen:

- `check-in` — opens the check-in flow for the vault.
- `withdraw` — opens the withdrawal flow for the vault.
- `view-details` — opens the vault detail screen.
- `manage-beneficiary` — opens the beneficiary management screen.
- `preview` — opens the read-only vault preview.

If the user is not authenticated, the app should defer navigation until authentication completes.

**Required platform configuration**

Custom schemes are registered via an `<intent-filter>` in `mobile/android/app/src/main/AndroidManifest.xml`. Because custom schemes cannot be verified, no `android:autoVerify` is used for these:

```xml
<!-- ttllegacy://vault/{vault_id}/{action} -->
<intent-filter>
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data android:scheme="ttllegacy" android:host="vault" />
</intent-filter>

<!-- ttl-legacy://vault/{vault_id}/{action} -->
<intent-filter>
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data android:scheme="ttl-legacy" android:host="vault" />
</intent-filter>
```

### HTTPS App Link: `https://ttl-legacy.app/vaults/{vault_id}/{action}`

```
https://ttl-legacy.app/vaults/{vault_id}/{action}
```

**Parameters**

| Parameter | Location | Description |
| --- | --- | --- |
| `vault_id` | path segment | The vault identifier. |
| `action` | path segment | One of `check-in`, `withdraw`, `view-details`, `manage-beneficiary`, `preview`. |

**Example**

```
https://ttl-legacy.app/vaults/abc123/check-in
```

**Expected app behavior**

Same routing as the custom-scheme links above: the app extracts `vault_id` and `action` and navigates to the corresponding vault screen. Because this is a verified App Link, the OS opens the app directly (rather than a browser) when the app is installed.

**Required platform configuration**

This is an Android App Link and must be declared with `android:autoVerify="true"` so the OS verifies domain ownership against `assetlinks.json`:

```xml
<!-- https://ttl-legacy.app/vaults/{vault_id}/{action} -->
<intent-filter android:autoVerify="true">
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data
        android:scheme="https"
        android:host="ttl-legacy.app"
        android:pathPrefix="/vaults/" />
</intent-filter>
```

**`assetlinks.json` requirement**

For `android:autoVerify="true"` to succeed, the domain must serve a Digital Asset Links file at:

```
https://ttl-legacy.app/.well-known/assetlinks.json
```

It must contain the app's package name and the SHA-256 fingerprint(s) of the signing certificate(s):

```json
[
  {
    "relation": ["delegate_permission/common.handle_all_urls"],
    "target": {
      "namespace": "android_app",
      "package_name": "app.ttllegacy",
      "sha256_cert_fingerprints": [
        "AA:BB:CC:...:ZZ"
      ]
    }
  }
]
```

If verification fails, the link falls back to opening in a browser.

### HTTPS App Link: `https://ttl-legacy.app/accept?vault_id=...`

```
https://ttl-legacy.app/accept?vault_id={vault_id}
```

**Parameters**

| Parameter | Location | Description |
| --- | --- | --- |
| `vault_id` | query string | The vault identifier for the beneficiary acceptance flow. |

**Example**

```
https://ttl-legacy.app/accept?vault_id=abc123
```

**Expected app behavior**

Opens the beneficiary acceptance flow for the given vault. If the user is not authenticated, the app defers navigation until authentication completes.

**Required platform configuration**

```xml
<!-- https://ttl-legacy.app/accept?vault_id=... -->
<intent-filter android:autoVerify="true">
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data
        android:scheme="https"
        android:host="ttl-legacy.app"
        android:pathPrefix="/accept" />
</intent-filter>
```

This App Link also relies on the same `https://ttl-legacy.app/.well-known/assetlinks.json` file described above.

---

## iOS

### Universal Link: `https://ttl-legacy.app/vaults/{vault_id}/invite`

```
https://ttl-legacy.app/vaults/{vault_id}/invite
```

**Parameters**

| Parameter | Location | Description |
| --- | --- | --- |
| `vault_id` | path segment | The vault identifier. |

**Example**

```
https://ttl-legacy.app/vaults/abc123/invite
```

**Expected app behavior**

Opens the vault invite screen for the given vault. If the user is not authenticated, navigation is deferred until authentication completes.

**Required platform configuration**

Universal Links require the **Associated Domains** capability with the `applinks:` entitlement:

```xml
<!-- ios/Runner/Runner.entitlements -->
<key>com.apple.developer.associated-domains</key>
<array>
    <string>applinks:ttl-legacy.app</string>
</array>
```

The domain must serve an Apple App Site Association file at `https://ttl-legacy.app/.well-known/apple-app-site-association` (served as `application/json`, no `.json` extension) listing the app's Team ID + bundle ID and the `/vaults/*` path.

### Universal Link: `https://ttl-legacy.app/vaults/{vault_id}/accept?token=...`

```
https://ttl-legacy.app/vaults/{vault_id}/accept?token={token}
```

**Parameters**

| Parameter | Location | Description |
| --- | --- | --- |
| `vault_id` | path segment | The vault identifier. |
| `token` | query string | The beneficiary acceptance token. |

**Example**

```
https://ttl-legacy.app/vaults/abc123/accept?token=xyz789
```

**Expected app behavior**

Opens the beneficiary acceptance flow for the given vault, passing the `token` to the acceptance handler. If the user is not authenticated, navigation is deferred until authentication completes.

**Required platform configuration**

Covered by the same `applinks:ttl-legacy.app` Associated Domains entitlement and `apple-app-site-association` file described above (the `/vaults/*` path pattern covers `/accept`).

### Universal Link: `https://ttl-legacy.app/vaults/{vault_id}/preview`

```
https://ttl-legacy.app/vaults/{vault_id}/preview
```

**Parameters**

| Parameter | Location | Description |
| --- | --- | --- |
| `vault_id` | path segment | The vault identifier. |

**Example**

```
https://ttl-legacy.app/vaults/abc123/preview
```

**Expected app behavior**

Opens the read-only vault preview for the given vault.

**Required platform configuration**

Covered by the same `applinks:ttl-legacy.app` Associated Domains entitlement and `apple-app-site-association` file described above.

### Custom scheme: `ttllegacy://vault/{vault_id}/{action}`

```
ttllegacy://vault/{vault_id}/{action}
```

**Parameters**

| Parameter | Location | Description |
| --- | --- | --- |
| `vault_id` | path segment | The vault identifier. |
| `action` | path segment | The vault action to perform. |

**Example**

```
ttllegacy://vault/abc123/check-in
```

**Expected app behavior**

The app parses the URI, extracts `vault_id` and `action`, and routes to the corresponding vault screen. If the user is not authenticated, navigation is deferred until authentication completes.

**Required platform configuration**

Custom URL schemes on iOS are registered in `Info.plist` under `CFBundleURLTypes`:

```xml
<key>CFBundleURLTypes</key>
<array>
    <dict>
        <key>CFBundleURLName</key>
        <string>app.ttllegacy.deeplink</string>
        <key>CFBundleURLSchemes</key>
        <array>
            <string>ttllegacy</string>
        </array>
    </dict>
</array>
```

### URL handling in the app

Universal Links and custom-scheme URLs are dispatched through `UniversalLinkRouter.swift`, with the app lifecycle entry points in `AppDelegate.swift`:

- **Universal Links** arrive via `application(_:continue:restorationHandler:)` (the `NSUserActivity` with `activityType == NSUserActivityTypeBrowsingWeb`). `AppDelegate` forwards the `NSUserActivity`'s `webpageURL` to `UniversalLinkRouter`, which matches the path (`/vaults/{vault_id}/invite`, `/vaults/{vault_id}/accept`, `/vaults/{vault_id}/preview`) and produces the corresponding navigation intent.
- **Custom schemes** arrive via `application(_:open:options:)`. `AppDelegate` forwards the `URL` to `UniversalLinkRouter`, which parses `ttllegacy://vault/{vault_id}/{action}` and produces the corresponding navigation intent.

In both cases, if the user is not authenticated, the router stores the pending navigation and resumes it after authentication completes.

---

## Discrepancy with the issue

Issue #2106 names the following schemes:

- `ttl://checkin/{vault_id}`
- `ttl://vault/{id}`
- `ttl://passkey/register`
- `ttl://release/{id}`

These `ttl://` schemes are **not** what the apps implement. The implemented schemes are:

- `ttllegacy://vault/{vault_id}/{action}` (Android + iOS)
- `ttl-legacy://vault/{vault_id}/{action}` (Android)
- `https://ttl-legacy.app/vaults/{vault_id}/{action}` (Android App Link)
- `https://ttl-legacy.app/accept?vault_id=...` (Android App Link)
- `https://ttl-legacy.app/vaults/{vault_id}/invite` (iOS Universal Link)
- `https://ttl-legacy.app/vaults/{vault_id}/accept?token=...` (iOS Universal Link)
- `https://ttl-legacy.app/vaults/{vault_id}/preview` (iOS Universal Link)

There is no `ttl://passkey/register` or `ttl://release/{id}` handler in the current codebase. Passkey registration is handled through the in-app flow described in [docs/mobile-passkey-flow.md](./mobile-passkey-flow.md), not via a deep link.

If the `ttl://` schemes are intended to be added, they must be registered in both `AndroidManifest.xml` (as `<intent-filter>` blocks) and `Info.plist` (`CFBundleURLTypes`), and routed in `UniversalLinkRouter.swift` / the Android deep-link handler. This document should be updated at that time.

---

## See also

- [docs/mobile-passkey-flow.md](./mobile-passkey-flow.md) — passkey registration and authentication flow.
- `mobile/android/app/src/main/AndroidManifest.xml` — Android intent filters.
- `ios/Runner/Runner.entitlements` — iOS Associated Domains entitlement.
- `UniversalLinkRouter.swift` / `AppDelegate.swift` — iOS URL routing.