# ADR-009: Stock image provider and licence

Status: proposed; implemented in Prompt 42. ARCHITECTURE §3.6 (`ImageSource`) is unchanged; the carry-over items
are listed in "What ARCHITECTURE needs" below and in `reports.md`. The provider's terms are as read on 2026-10-17;
they are the provider's to change, and nothing here is legal advice (see "Needs a person").

## Question

ARCHITECTURE §3.6 says `ImageSource` has a `stock` implementation over "a licensed stock API, key in
`SecretStore`". Which provider, and does its licence let a product we ship (open source, installed on people's
computers, no Vellum server in the path) offer it?

## What a provider must allow

1. A key per person, which the person gets for free and stores in their own keychain. Vellum ships no key and runs
   no proxy (Invariant 10; "no account requirement for any local feature").
2. Downloading the picture and keeping it in the repository. A design file must not stop rendering when a stock
   site is down or a URL expires, so a provider that requires hotlinking cannot be used.
3. A licence that lets people use the picture in their own products without paying us or the provider, and
   attribution that can be kept as data rather than shown on screen.
4. Terms for the API that an application the person runs on their own computer, with their own key, can meet.

## Candidates (terms read 2026-10-17)

| Provider | Verdict | Why |
|---|---|---|
| **Pixabay** | **Chosen** | Free key for anyone with an account. Its API terms ask for results to be cached for 24 hours and say permanent hotlinking is not allowed: pictures must be downloaded first, which is what Vellum does. Attribution is asked for "where results are displayed". The Pixabay Content License allows commercial use, with or without attribution, and bars selling or distributing a picture on a standalone basis (with no creative effort applied), using pictures that contain trademarks or logos in relation to goods and services, and using recognisable people in an immoral or illegal way. Only the full licence is binding; the summary page was read. `largeImageURL` (up to 1280 px) needs no special approval. |
| Unsplash | Rejected | Its API terms require image URLs to be used from the API's own links ("hotlinking") and every download to be reported, and say "You may not share your Credentials with any third party", which is what a per-person key in a distributed app asks of the terms. A repository copy of the picture is not what those terms describe. |
| Pexels | Rejected for now | The API page says new API keys are currently not being issued. Its licence would otherwise suit. The port makes this a one-file swap if keys open up. |

## Decision

`vellum-images-stock` implements `ImageSource` over Pixabay's search API.

- The person's key is stored in the keychain under `stock.pixabay` (`SecretStore`), never in a file, setting,
  log, error, MCP result or URL that is stored. The request carries a `{secret}` placeholder; only the HTTP
  adapter substitutes the key, at the moment of sending, and errors are payload-free.
- Search results are kept for 24 hours per query (the API terms), at most 64 entries, in memory only.
- A chosen picture is downloaded once into the document's `.assets` folder like any other asset. The design never
  links to the provider afterwards. Downloads are accepted only from `pixabay.com` addresses that a search result
  named, over https, as JPEG, PNG or WEBP, at most 12 MB.
- Every layer filled from a stock picture gets `metadata.attribution` with the provider, author, author page,
  picture page, licence name and licence page. Replacing the picture with a local one removes it. The UI shows
  attribution from this data (C30 "Stock"); the data is the source of truth, not a rendered credit.
- Reaching the provider is network access by an agent, so each `fill_image` with `query` asks the broker once
  (`Action::Network` for `https://pixabay.com`), covering the search and the download. It is "ask every time".
- The provider sits behind `ImageSource` and the HTTP behind `HttpFetch`; swapping to another provider is one new
  adapter and a contract-suite pass.

## What this does not settle

- Pixabay's documentation does not say in so many words that an individual's key may be used in software that
  person installs. Reading its API page, the terms are met (key per use, cached, downloaded not hotlinked,
  attribution kept), but a person should confirm with Pixabay before the stock tab ships in a public release.
  If they decline, the fallback is the `local` source only and a different provider later.
- Pixabay may withhold full-size originals (`fullHDURL`, `imageURL`) without "full API access"; we use only the 1280 px
  version and do not ask for more.
- The licence summary covers the pictures; people shown in them, brands and trademarks in them are not cleared by
  it. The UI should say so next to the attribution (a Prompt 55 copy decision).

## What ARCHITECTURE needs (not edited here)

§3.6 `ImageSource` should name `origin()` (what the person is asked to allow), `search` and `fetch` as
implemented; §3.1 should list `HttpFetch` (plain https GET, `{secret}` placeholder, address rules) with its one
adapter in `vellum-model-http` and its contract suite; §3.6 `Importer` is `recognizes` + `import` returning node
specs and assets by reference (`import:N`), with no ids and no file access.
