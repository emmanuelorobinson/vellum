---
title: "Providers and keys"
description: "The model providers the built-in agent can use, their endpoints, the credential each one needs, and where each key is read from."
---

The built-in agent talks to a model through a provider. You choose the provider and the model, and you supply the key. {{brand}} never
marks up tokens, and it never sends your conversation anywhere except to the provider you chose.

## Chat Completions providers

These presets use the Chat Completions protocol. Each supplies an endpoint address and a key. A preset does not claim that a model is
available, what its context size is, or whether your key works.

| Provider | Notes |
| --- | --- |
| OpenRouter | The default. Its default model is `deepseek/deepseek-v4.1-flash` |
| DeepSeek | |
| xAI | |
| Moonshot (Kimi) | |
| Together | |
| Fireworks | |
| Z.AI | |
| OpenCode Zen | |
| Ollama | Local. Needs no key |
| LM Studio | Local. Needs no key |

A custom provider that speaks Chat Completions, or the Responses protocol, takes its full endpoint address.

## Direct protocols

Two of the larger providers have their own protocols. OpenAI uses the Responses protocol. Google uses GenerateContent, with
the AI Studio endpoint described below. Anthropic's API and the deployments listed next use the Messages protocol.

## Messages providers

The Messages protocol is spoken by these deployments, each with its own endpoint:

| Deployment | Endpoint | Credential |
| --- | --- | --- |
| Anthropic | `https://api.anthropic.com/v1/messages` | An API key |
| Amazon Bedrock | `https://bedrock-runtime.<region>.amazonaws.com/anthropic/v1/messages` | A Bedrock key or token |
| Google Vertex | The Vertex Anthropic endpoint for your project and location | An OAuth token that your host obtains and stores |
| Microsoft Foundry | `https://<resource>.services.ai.azure.com/anthropic/v1/messages` | A Foundry API key |
| Custom | The full URL you give | A key, or none for a local server you run |

Gemini uses the AI Studio endpoint, and its key is sent in the `x-goog-api-key` header. A key is never placed in a URL.

## Where each key is read from

| Situation | Where the key comes from |
| --- | --- |
| On your computer | The operating system's keychain, under the service `vellum`, with the provider's id as the account |
| In continuous integration | An environment variable named `VELLUM_<PROVIDER>_API_KEY`, read only when `CI` is set. The conventional names are read too |
| Installed Claude Code or Codex | The program's own sign-in. {{brand}} never reads or writes it |

Keys are never written to a design file, a session, a log or a usage report. A key that the model echoes back is redacted before it is
shown.

## Choosing a model

Pass the provider and the model together, as `provider/model`, for example `openrouter/deepseek/deepseek-v4.1-flash`. Pass `--context-window`
and `--vision` when you know the model's limits, because {{brand}} does not guess them from the provider's name.

The [command line](/docs/reference/cli) page lists the options for `prompt` and `models`.
