import type {
  CsrfTokenResponse,
  DeviceToken,
  ErrorResponse,
  HealthResponse,
  ReadyResponse,
  RegisterTokenRequest,
  ReminderPreferences,
  SetPreferencesRequest,
  UpgradeProposal,
} from "./generated";

export type ApiMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS";

export interface ApiClientOptions {
  baseUrl?: string;
  credentials?: RequestCredentials;
  csrfCookieName?: string;
  csrfHeaderName?: string;
}

export class ApiError extends Error {
  readonly status: number;
  readonly body?: unknown;

  constructor(message: string, status: number, body?: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.body = body;
  }
}

const DEFAULT_CSRF_COOKIE_NAME = "__Host-csrf";
const DEFAULT_CSRF_HEADER_NAME = "x-csrf-token";

function resolveBaseUrl(baseUrl?: string): string {
  if (baseUrl && baseUrl.length > 0) {
    return baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl;
  }

  if (typeof window !== "undefined" && window.location) {
    return window.location.origin;
  }

  return "";
}

function readCookie(name: string): string | null {
  if (typeof document === "undefined") {
    return null;
  }

  const match = document.cookie
    .split("; ")
    .find((entry) => entry.startsWith(`${name}=`));

  if (!match) {
    return null;
  }

  return decodeURIComponent(match.slice(name.length + 1));
}

async function parseApiResponse<T>(response: Response): Promise<T> {
  const text = await response.text();

  if (text.length === 0) {
    return undefined as T;
  }

  try {
    return JSON.parse(text) as T;
  } catch {
    return text as unknown as T;
  }
}

export class ApiClient {
  private readonly baseUrl: string;
  private readonly credentials: RequestCredentials;
  private readonly csrfCookieName: string;
  private readonly csrfHeaderName: string;

  constructor(options: ApiClientOptions = {}) {
    this.baseUrl = resolveBaseUrl(options.baseUrl);
    this.credentials = options.credentials ?? "same-origin";
    this.csrfCookieName = options.csrfCookieName ?? DEFAULT_CSRF_COOKIE_NAME;
    this.csrfHeaderName = options.csrfHeaderName ?? DEFAULT_CSRF_HEADER_NAME;
  }

  public async getHealth(): Promise<HealthResponse> {
    return this.request<HealthResponse>("/health");
  }

  public async getReady(): Promise<ReadyResponse> {
    return this.request<ReadyResponse>("/ready");
  }

  public async getCsrfToken(): Promise<CsrfTokenResponse> {
    return this.request<CsrfTokenResponse>("/api/csrf-token");
  }

  public async getReminderPreferences(vaultId: string): Promise<ReminderPreferences> {
    return this.request<ReminderPreferences>(`/api/vaults/${vaultId}/reminder-preferences`);
  }

  public async setReminderPreferences(
    vaultId: string,
    payload: SetPreferencesRequest,
    headers?: Record<string, string>,
  ): Promise<ReminderPreferences> {
    return this.request<ReminderPreferences>(`/api/vaults/${vaultId}/reminder-preferences`, {
      method: "POST",
      body: JSON.stringify(payload),
      headers,
    });
  }

  public async registerDeviceToken(payload: RegisterTokenRequest): Promise<DeviceToken> {
    return this.request<DeviceToken>("/api/notifications/register", {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  public async getUpgradeProposals(): Promise<UpgradeProposal[]> {
    return this.request<UpgradeProposal[]>("/api/admin/upgrade-proposals");
  }

  public async approveUpgradeProposal(id: string | number): Promise<UpgradeProposal> {
    return this.request<UpgradeProposal>(
      `/api/admin/upgrade-proposals/${encodeURIComponent(String(id))}/approve`,
      { method: "POST" },
    );
  }

  public async rejectUpgradeProposal(id: string | number): Promise<UpgradeProposal> {
    return this.request<UpgradeProposal>(
      `/api/admin/upgrade-proposals/${encodeURIComponent(String(id))}/reject`,
      { method: "POST" },
    );
  }

  public async getContractAdmin(): Promise<string> {
    const response = await this.request<{ admin: string }>("/api/admin/contract-admin");
    return response.admin;
  }

  public async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const method = (init.method ?? "GET").toUpperCase() as ApiMethod;
    const isMutatingMethod = !["GET", "HEAD", "OPTIONS"].includes(method);
    const headers = new Headers(init.headers ?? {});

    if (!headers.has("Accept")) {
      headers.set("Accept", "application/json");
    }

    if (isMutatingMethod && !headers.has(this.csrfHeaderName)) {
      const csrfToken = readCookie(this.csrfCookieName) ?? (await this.getCsrfToken()).csrf_token;
      if (csrfToken) {
        headers.set(this.csrfHeaderName, csrfToken);
      }
    }

    if (init.body && !(init.body instanceof FormData) && !headers.has("Content-Type")) {
      headers.set("Content-Type", "application/json");
    }

    const response = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      method,
      headers,
      credentials: this.credentials,
    });

    if (!response.ok) {
      const errorBody = await parseApiResponse<ErrorResponse | unknown>(response);
      const message =
        typeof errorBody === "object" && errorBody && "error" in errorBody
          ? String(errorBody.error)
          : `Request failed with status ${response.status}`;

      throw new ApiError(message, response.status, errorBody);
    }

    return parseApiResponse<T>(response);
  }
}

export function createApiClient(options: ApiClientOptions = {}): ApiClient {
  return new ApiClient(options);
}

export const apiClient = new ApiClient();
export default apiClient;
