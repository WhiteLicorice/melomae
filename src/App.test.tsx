import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import App from "./App";

describe("App", () => {
  it("renders the Melomae title", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { name: "Melomae" }),
    ).toBeInTheDocument();
  });
});
