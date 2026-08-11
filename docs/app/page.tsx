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
      <pre>
        <code>cargo install --path .</code>
      </pre>
    </>
  );
}
