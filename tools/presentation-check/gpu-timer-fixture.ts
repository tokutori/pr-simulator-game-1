import { vi } from "vitest";

export function gpuTimerFixture() {
  let current: WebGLQuery | null = null;
  const extension = { TIME_ELAPSED_EXT: 10, GPU_DISJOINT_EXT: 11, QUERY_COUNTER_BITS_EXT: 12 };
  const gl = {
    CURRENT_QUERY: 1, QUERY_RESULT_AVAILABLE: 2, QUERY_RESULT: 3,
    getExtension: vi.fn(() => extension as typeof extension | null),
    getQuery: vi.fn((_target: number, parameter: number): unknown => parameter === 12 ? 64 : current),
    createQuery: vi.fn((): WebGLQuery => ({})),
    beginQuery: vi.fn((_target: number, query: WebGLQuery) => { current = query; }),
    endQuery: vi.fn(() => { current = null; }),
    deleteQuery: vi.fn(),
    getParameter: vi.fn(() => false),
    getQueryParameter: vi.fn((_query: WebGLQuery, parameter: number): unknown => parameter === 2 ? true : 20_000_000)
  };
  return { gl, context: gl as unknown as WebGL2RenderingContext, foreign: (query: WebGLQuery | null) => { current = query; } };
}
