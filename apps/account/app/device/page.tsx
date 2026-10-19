import { headers } from "next/headers";
import { accountAuth } from "../../lib/auth";
import { AuthForm } from "../AuthForm";
export const dynamic = "force-dynamic";
export default async function DevicePage({
  searchParams,
}: {
  searchParams: Promise<{ user_code?: string }>;
}) {
  const query = await searchParams;
  let signedIn = false;
  try {
    signedIn = Boolean(await accountAuth().getSession(await headers()));
  } catch {
    return (
      <main>
        <h1>Account service unavailable</h1>
        <p>
          Try again when the service is available. Local editing and agents work without an account.
        </p>
      </main>
    );
  }
  return (
    <AuthForm
      deviceCode={typeof query.user_code === "string" ? query.user_code.slice(0, 191) : ""}
      signedIn={signedIn}
    />
  );
}
