import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { ResourceSample, Resources } from "@/api/types";
import { mockApi, renderWithProviders } from "@/test/render";
import { ResourcesSection } from "./ResourcesSection";

const GB = 1024 ** 3;

function sample(at: number, pley_cpu: number, cpu: number): ResourceSample {
  return { at, cpu, pley_cpu, memory: 8 * GB, pley_memory: GB / 2 };
}

function resources(samples: ResourceSample[], available = true): Resources {
  return { available, interval_ms: 2_000, cpus: 8, memory_total: 32 * GB, samples };
}

describe("ResourcesSection", () => {
  it("shows pley's and the machine's processors and memory, then asks only for newer samples", async () => {
    const answers = [resources([sample(1_000, 2.5, 20), sample(3_000, 12, 34)]), resources([sample(5_000, 40, 61)])];
    const requests = mockApi({ "GET /api/system/resources": () => answers.shift() ?? resources([]) });
    renderWithProviders(<ResourcesSection />);

    const cpu = await screen.findByRole("img", { name: "CPU: pley 12%, System 34%" });
    expect(cpu).toBeInTheDocument();
    expect(screen.getByText("8 processors")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Memory: pley 512 MB, System 8.0 GB" })).toBeInTheDocument();
    expect(screen.getByText("32.0 GB in all")).toBeInTheDocument();

    // The next poll carries the newest sample's time, and its answer joins the others.
    expect(await screen.findByRole("img", { name: "CPU: pley 40%, System 61%" }, { timeout: 4_000 })).toBeInTheDocument();
    const polls = requests.filter((r) => r.path.startsWith("/api/system/resources"));
    expect(polls[0]?.path).toBe("/api/system/resources");
    expect(polls[1]?.path).toBe("/api/system/resources?since=3000");
  });

  it("reads out each sample from the keyboard", async () => {
    const user = userEvent.setup();
    mockApi({ "GET /api/system/resources": () => resources([sample(1_000, 2.5, 20), sample(3_000, 12, 34)]) });
    renderWithProviders(<ResourcesSection />);
    const cpu = await screen.findByRole("img", { name: /^CPU/ });
    await user.click(cpu);
    await user.keyboard("{ArrowLeft}");
    await waitFor(() => expect(cpu).toHaveTextContent("2.5%pley20%System"));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(cpu).not.toHaveTextContent("pley"));
  });

  it("says so where the system can't be measured", async () => {
    mockApi({ "GET /api/system/resources": () => resources([], false) });
    renderWithProviders(<ResourcesSection />);
    expect(await screen.findByText("The processors and memory can't be measured on this system.")).toBeInTheDocument();
  });
});
