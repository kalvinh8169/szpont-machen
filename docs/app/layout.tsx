import type { Metadata } from "next";
import "./globals.css";
import Nav from "./Nav";

export const metadata: Metadata = {
  title: "szpont machen — docs",
  description:
    "szpont machen is a terminal manager for AI CLI tool sessions: Claude Code, Codex CLI and Kimi Code.",
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>
        <header>
          <a className="brand" href="./">
            szpont machen 🐝
          </a>
          <Nav />
        </header>
        <main>{children}</main>
        <footer>szpont — The Unlicense (public domain)</footer>
      </body>
    </html>
  );
}
