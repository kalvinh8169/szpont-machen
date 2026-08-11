export default function Overview() {
  return (
    <>
      <h1>szpont machen</h1>
      <pre className="bee">
        <span className="bee-wings">{`      __    __
      \\ \\  / /
       \\ \\/ /`}</span>
        {"\n"}
        <span className="bee-body">{`     .-=(o o)=-.`}</span>
        {"\n"}
        <span className="bee-body">{`  ==[ `}</span>
        <span className="bee-stripe-a">≡≡</span>
        <span className="bee-stripe-b">≡≡</span>
        <span className="bee-stripe-a">≡≡</span>
        <span className="bee-stripe-b">≡≡</span>
        <span className="bee-stripe-a">≡≡</span>
        <span className="bee-body">{` ]==>`}</span>
        {"\n"}
        <span className="bee-body">{`     '-=(___)=-'`}</span>
      </pre>
      <p>
        A terminal manager for AI CLI tool sessions —{" "}
        <strong>Claude Code</strong>, <strong>Codex CLI</strong> and{" "}
        <strong>Kimi Code</strong>. Pronounced <em>&quot;shpont
        mah-khen&quot;</em>.
      </p>

      <h2>Install</h2>
      <p>Homebrew:</p>
      <pre>
        <code>brew install tjzel/tap/szpont</code>
      </pre>
      <p>Shell installer (prebuilt binaries, self-updates via szpont-update):</p>
      <pre>
        <code>
          curl -LsSf
          https://github.com/tjzel/szpont-machen/releases/latest/download/szpont-installer.sh
          | sh
        </code>
      </pre>
      <p>crates.io (needs Rust 1.91+ and a C compiler):</p>
      <pre>
        <code>cargo install szpont</code>
      </pre>
    </>
  );
}
