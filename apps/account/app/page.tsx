import { brand } from "@vellum/config";

export default function Page() {
  return (
    <main>
      <h1>{brand.name}</h1>
      <p>{brand.tagline}</p>
    </main>
  );
}
