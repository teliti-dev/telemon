import * as React from "react"

/** Galat di dalam extension tidak boleh menjatuhkan aplikasi: tampilkan pesan kecil di tempatnya. */
export class ExtensionBoundary extends React.Component<{ name: string; children: React.ReactNode; fallback: (name: string) => React.ReactNode }, { failed: boolean }> {
  state = { failed: false }
  static getDerivedStateFromError() {
    return { failed: true }
  }
  componentDidCatch(error: unknown) {
    console.error(`Extension ${this.props.name} gagal`, error)
  }
  render() {
    return this.state.failed ? this.props.fallback(this.props.name) : this.props.children
  }
}
