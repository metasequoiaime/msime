// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AiSkinGeneration } from "../../../../packages/ui/src/keyboard/ai-skin-generation";
import type {
  AiSkinClient,
  AiSkinProgress,
  AiSkinProposal,
  CustomSkinLibraryClient,
} from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((accept, refuse) => {
    resolve = accept;
    reject = refuse;
  });
  return { promise, resolve, reject };
}

function host() {
  const generation = deferred<AiSkinProposal[]>();
  const registrations: {
    listener: (progress: AiSkinProgress) => void;
    pending: ReturnType<typeof deferred<() => void>>;
    stop: ReturnType<typeof vi.fn<() => void>>;
  }[] = [];
  const client = {
    generate: vi.fn((_requestId: string, _prompt: string) => generation.promise),
    cancel: vi.fn(async () => undefined),
    onProgress: vi.fn((listener: (progress: AiSkinProgress) => void) => {
      const pending = deferred<() => void>();
      registrations.push({ listener, pending, stop: vi.fn() });
      return pending.promise;
    }),
  };
  return { client, generation, registrations };
}

const library: CustomSkinLibraryClient = {
  load: async () => [],
  mutate: async () => [],
};

function view(client: AiSkinClient) {
  return <AiSkinGeneration client={client} library={library} onUse={vi.fn()} onClose={vi.fn()} />;
}

test("releases late progress registrations after StrictMode replay and unmount", async () => {
  const current = host();
  const mounted = render(<StrictMode>{view(current.client)}</StrictMode>);
  expect(current.registrations).toHaveLength(2);
  mounted.unmount();

  await act(async () => {
    for (const registration of current.registrations)
      registration.pending.resolve(registration.stop);
  });
  for (const registration of current.registrations)
    expect(registration.stop).toHaveBeenCalledOnce();
});

test("releases an established progress registration exactly once on unmount", async () => {
  const current = host();
  const mounted = render(view(current.client));
  const registration = current.registrations[0];
  await act(async () => registration.pending.resolve(registration.stop));
  expect(registration.stop).not.toHaveBeenCalled();

  mounted.unmount();
  expect(registration.stop).toHaveBeenCalledOnce();
});

test("old clients cannot update progress and their late registrations are released", async () => {
  const old = host();
  const current = host();
  const mounted = render(view(old.client));
  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  const requestId = old.client.generate.mock.calls[0][0];
  mounted.rerender(view(current.client));

  await act(async () => {
    for (const registration of old.registrations) {
      registration.pending.resolve(registration.stop);
      registration.listener({ requestId, completed: 3 });
    }
  });
  for (const registration of old.registrations) expect(registration.stop).toHaveBeenCalledOnce();
  expect(screen.getByRole("status").textContent).toContain("0/3");

  act(() => current.registrations[0].listener({ requestId, completed: 2 }));
  expect(screen.getByRole("status").textContent).toContain("2/3");
});

test("progress follows the current request and ignores cancelled request listeners", async () => {
  const current = host();
  render(view(current.client));
  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  const firstId = current.client.generate.mock.calls[0][0];
  const firstRegistration = current.registrations.at(-1)!;
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(current.client.cancel).toHaveBeenCalledWith(firstId);
  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  const secondId = current.client.generate.mock.calls[1][0];
  const secondRegistration = current.registrations.at(-1)!;
  expect(secondId).not.toBe(firstId);

  await act(async () => {
    firstRegistration.pending.resolve(firstRegistration.stop);
    firstRegistration.listener({ requestId: secondId, completed: 3 });
    secondRegistration.listener({ requestId: firstId, completed: 2 });
  });
  expect(firstRegistration.stop).toHaveBeenCalledOnce();
  expect(screen.getByRole("status").textContent).toContain("0/3");

  act(() => secondRegistration.listener({ requestId: secondId, completed: 1 }));
  expect(screen.getByRole("status").textContent).toContain("1/3");
});

test("progress registration failure stays handled and generation can finish", async () => {
  const current = host();
  render(view(current.client));
  await act(async () => {
    current.registrations[0].pending.reject(new Error("synthetic progress unavailable"));
  });

  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  expect(current.client.generate).toHaveBeenCalledOnce();
  await act(async () => {
    current.registrations
      .at(-1)!
      .pending.reject(new Error("synthetic request progress unavailable"));
    current.generation.resolve([]);
  });
  expect(screen.queryByRole("status")).toBeNull();
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "抽三张皮肤" }).disabled).toBe(
    false,
  );
});
