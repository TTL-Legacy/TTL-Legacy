import { renderHook, act } from '@testing-library/react';
import { useWallet } from '../../hooks/useWallet';

// Mock Freighter API
const mockFreighter = {
  requestAccess: jest.fn(),
  getPublicKey: jest.fn(),
  signTransaction: jest.fn(),
};

Object.defineProperty(window, 'freighter', {
  value: mockFreighter,
  writable: true,
  configurable: true,
});

describe('useWallet', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    sessionStorage.clear();
  });

  it('initializes with no wallet connected', () => {
    const { result } = renderHook(() => useWallet());
    expect(result.current.isConnected).toBe(false);
    expect(result.current.address).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('successfully connects to wallet when Freighter is available', async () => {
    mockFreighter.requestAccess.mockResolvedValue('success');
    mockFreighter.getPublicKey.mockResolvedValue('GTEST123456789');

    const { result } = renderHook(() => useWallet());

    await act(async () => {
      await result.current.connect();
    });

    expect(result.current.isConnected).toBe(true);
    expect(result.current.address).toBe('GTEST123456789');
    expect(result.current.error).toBeNull();
  });

  it('handles wallet not installed error', async () => {
    mockFreighter.requestAccess.mockRejectedValue(new Error('Freighter not installed'));

    const { result } = renderHook(() => useWallet());

    await act(async () => {
      try {
        await result.current.connect();
      } catch (e) {
        // Error is expected
      }
    });

    expect(result.current.isConnected).toBe(false);
    expect(result.current.error).toBe('Freighter not installed');
  });

  it('handles user rejection of wallet connection', async () => {
    mockFreighter.requestAccess.mockRejectedValue(new Error('User rejected'));

    const { result } = renderHook(() => useWallet());

    await act(async () => {
      try {
        await result.current.connect();
      } catch (e) {
        // Error is expected
      }
    });

    expect(result.current.isConnected).toBe(false);
    expect(result.current.error).toBe('User rejected');
  });

  it('persists connected address in session storage', async () => {
    mockFreighter.requestAccess.mockResolvedValue('success');
    mockFreighter.getPublicKey.mockResolvedValue('GTEST987654321');

    const { result } = renderHook(() => useWallet());

    await act(async () => {
      await result.current.connect();
    });

    expect(sessionStorage.getItem('wallet_address')).toBe('GTEST987654321');
  });

  it('restores wallet address from session storage on mount', () => {
    sessionStorage.setItem('wallet_address', 'GSAVED123456789');

    const { result } = renderHook(() => useWallet());

    expect(result.current.address).toBe('GSAVED123456789');
    expect(result.current.isConnected).toBe(true);
  });

  it('allows disconnecting from wallet', async () => {
    mockFreighter.requestAccess.mockResolvedValue('success');
    mockFreighter.getPublicKey.mockResolvedValue('GTEST111111111');

    const { result } = renderHook(() => useWallet());

    await act(async () => {
      await result.current.connect();
    });

    expect(result.current.isConnected).toBe(true);

    await act(async () => {
      result.current.disconnect();
    });

    expect(result.current.isConnected).toBe(false);
    expect(result.current.address).toBeNull();
    expect(sessionStorage.getItem('wallet_address')).toBeNull();
  });
});
