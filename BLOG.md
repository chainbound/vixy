# vixy: Vibing the Ethereum EL and CL Proxy

![Vixy Header](vixy%20Vibing%20the%20Ethereum%20EL%20and%20CL%20Proxy/Gemini_Generated_Image_ce9n8oce9n8oce9n.jpeg)

At [chainbound](https://x.com/chainbound_), we like to experiment with fun stuff and since Claude launched Opus 4.5, it’s been hell lot of fun playing with it. On this episode of experimentation, we tried to one-shot an Ethereum Execution Layer and Consensus Layer proxy. We applied modern engineering practices as guardrails and wanted to see whether or not it can ship a production-ready service quickly.

## Backstory

As a company focused on blockchain development and research, we run tons of execution clients and consensus clients ourselves. The problem comes when nodes are out of sync or even dead. We need a way of automatic detection and backup for them. So we decided that we need to put a proxy that handles fail recovery for both EL and CL.

At first, we were going to integrate https://github.com/rainshowerLabs/blutgang for EL and https://github.com/sozu-proxy/sozu for CL. But https://x.com/merklefruit was like nahhhh, we can do better, why bother with having two dependencies when we can maybe one-shot a single binary to handle both cases. And I was like let’s do it!

## The Spec

1. Read config.toml file for EL and CL node list
2. Always check those lists to mark healthy nodes
3. Serve proxy endpoints (for EL HTTP and WS, and CL HTTP) that always try to point to a healthy node

And the health conditions are as follows:

- **EL**: `eth_getBlockNumber` returns a block number (in hex). Compare the block numbers between all active ELs, keep track of the highest one (aka “chain head”), and track the “lag” from that. Consider a node unhealthy if it’s lagging more than a configurable MAX_EL_LAG_BLOCKS value.
- **CL**: `/eth/v1/node/health` should return 200 OK, AND `/eth/v1/beacon/headers/head` must return a value in `/data/header/message/slot` (JSON pointer) which is the slot number. Same story here, track the CL chain head and keep track of lag, and consider unhealthy if lag > MAX_CL_LAG_SLOTS value.

## The Prompt

[The Initial Prompt PR](https://github.com/chainbound/vixy/pull/1)

Naively, most people will just copy the spec and then tell Claude to code the service directly. It might work, but the better thing to do would be to ask them to act as an engineer and make a plan for it. Hence we decided to create a file called AGENT.md in which we vibe-prompted the plan of what the AI agent would create.

One of the tips that I read online by https://x.com/mert/status/2009986072953126935, is to ask the AI agent to first draw a diagram of the architecture. We did that because it’s pretty much aligned with how we would create technical docs of a service. We would draw the architecture diagram of the whole service first.

![Architecture Diagram](vixy%20Vibing%20the%20Ethereum%20EL%20and%20CL%20Proxy/Screenshot_2026-01-15_at_22.24.46.png)

By drawing the architecture diagram of the whole service first, we could get a high-level view of how the agent was going to plan everything. We also had the advantage to fix the flow first before the agent was way ahead in the thinking.

After the diagram looked good, we asked them to continue with explaining the logic and make a step-by-step development plan todo.

The AGENT.md plan broke everything down into clear phases: project setup, config parsing, health checks, the proxy server itself, metrics, and so on. Each phase had clear deliverables and acceptance criteria. The AI could follow this blueprint autonomously without getting lost.

## The Guardrails

Even with development plan todos, there are just so many things that could go wrong with long-running AI agents. This is where our experience as software engineers can help as guardrails. Just like what https://x.com/lwastuargo/status/2006193951607578706 said, using modern engineering practices as guardrails is the best way to keep it sane and precise.

Here’s the thing about working with AI on code: it’s really good at writing code, but it needs constraints. Not because it sucks, but because without boundaries it’ll just keep going and you’ll end up with a mess. So we borrowed from the playbook of traditional software development and applied it as guardrails.

### Tests, Tests, Tests

First up was TDD. We told Claude: write the tests first, then make them pass. This is important because tests are the source of truth. When Claude writes code and says “it works,” how do you know? You run the tests. When Claude refactors something, how do you know it didn’t break? Tests still pass.

We wrote 72 unit tests throughout the project. Every feature started with failing tests. Red, green, refactor. This actually caught real bugs, like when unreachable nodes were being marked as healthy because of some weird edge case in our lag calculation. The test failed, we fixed it, moved on.

Then there’s BDD with Cucumber. This was clutch because BDD scenarios are basically plain English specs. “Given an EL node at block 1000, when the health check runs, then it should be marked healthy.” Super clear, no ambiguity. Claude understood exactly what we wanted, and we ended up with 16 scenarios that serve as both tests and documentation.

But unit tests with mocks only get you so far. You gotta test against the real thing. So we used [Kurtosis](https://kurtosis.com/) to spin up an actual Ethereum testnet with 4 EL nodes and 4 CL nodes. Ran 15 integration tests against real infrastructure. This caught bugs that unit tests missed, like we weren’t forwarding Content-Type headers and geth was returning HTTP 415. Oops. Fixed it before it ever hit production.

### Keep a Diary

Here’s something that people don’t talk about enough: AI has no long-term memory. But also, even within a single session, Claude compacts its memory when the context gets too long. It summarizes what happened earlier to make room for new stuff. This is great for performance, but you lose details.

So we kept a DIARY.md. Every bug we hit, every fix we made, every decision we took got documented in real-time. Not at the end of the day, but as we went. Each entry followed a simple structure:

- **What I did:** List of tasks completed in this phase
- **Challenges faced:** Problems that came up
- **How I solved it:** The actual solutions we implemented
- **What I learned:** Key takeaways from this phase
- **Mood:** How we felt (excited, frustrated, accomplished, etc.)

The diary entries were detailed. Like when we hit the Content-Type header bug during Kurtosis integration testing, the diary captured exactly what failed, how we debugged it, and how we fixed it. When we implemented WebSocket reconnection, it documented all the components we built, the tests we wrote, and the tricky type system issues we solved.

When Claude’s memory got compacted after hours of work, it could just read the diary and catch up on everything. “Oh right, we already tried that approach and it didn’t work because X.” Without the diary, Claude might suggest the same broken solution again. The diary kept both of us on the same page across the entire 8-hour development session.

This became our shared knowledge base. Hit a similar bug? Check the diary. Need to remember why we chose approach A over B? It’s in the diary. Plus, when it came time to write this blog post, we didn’t have to remember anything because it was all already written down. The diary literally wrote itself as we worked.

### Break It Down, Commit Often

We broke everything into tiny todos. Not “build the proxy server” but “parse incoming HTTP request” and “extract JSON-RPC method” and “select healthy node.” Small, specific tasks. Claude never got lost because it always knew the next immediate step.

And we committed after every completed phase. Not massive dumps of code, but digestible chunks. CI ran on every single commit: format check, clippy, tests, BDD scenarios. If anything failed, we stopped and fixed it. No accumulating tech debt, no “we’ll fix it later.” Fix it now or don’t move forward.

By the end we had 15+ commits, each one passing CI, each one a safe rollback point if something went wrong.

## The Execution

With AGENT.md plan and guardrails in place, we just… let Claude cook.

We worked phase by phase. Claude wrote tests, implemented features, ran CI, and committed. We guided when needed, clarified ambiguities, and reviewed the output. But mostly, we let it work autonomously.

It felt less like “using a tool” and more like pair programming with someone who’s really fast at the boring parts but needs you to make the judgment calls.

Some bugs happened. Unreachable nodes marked as healthy, route syntax changes in axum 0.8, WebSocket type mismatches. But the guardrails caught them early. Tests failed, we fixed them, moved on. No big drama.

## What We Built

In about 8 hours of work (spread across a day), we shipped Vixy, a production-ready Ethereum proxy that monitors node health, handles automatic failover to backup nodes, proxies both HTTP and WebSocket traffic, and exposes status + metrics endpoints.

**The Stats:**
- 85 unit tests, all passing
- 33 BDD scenarios (147 steps), all passing
- 17 integration tests against real Ethereum nodes via Kurtosis
- 60+ commits, each one passing CI
- ~4,400 lines of Rust, formatted, linted, documented
- 8 hours from empty repo to production-ready

## What We Learned

AI is a force multiplier, not a replacement. We still made the architectural decisions, designed the testing strategy, and guided implementation. Claude handled the grunt work (boilerplate, test cases, debugging). It’s like having a fast, thorough junior engineer who never gets tired but needs clear direction.

Guardrails are non-negotiable. Without TDD, BDD, integration tests, incremental todos, frequent commits, and diary writing, this would’ve gone off the rails. The guardrails aren’t “nice-to-haves,” they’re what make AI development reliable. Vague instructions get you vague results. Clear practices get you clear outcomes.

Good specs enable autonomy. AGENT.md with a clear architecture diagram and phase breakdown let Claude work autonomously for hours. We didn’t micromanage. We set direction, it executed.

Documentation is your future self’s best friend. DIARY.md was a knowledge base for both us and Claude. Hit a bug? Check the diary. Need to write a blog post? It’s already documented.

## The Bottom Line

Can you one-shot a production-ready service with AI? **Yes, but only with the right guardrails.**

We didn’t just throw a spec at Claude and hope for the best. We created a detailed plan, enforced strict practices (TDD, BDD, frequent commits), maintained documentation, tested against real systems, and ran CI on every commit.

The result: Vixy went from empty repo to production-ready in a single day, with comprehensive test coverage and zero known bugs.

Is Vixy perfect? No. There’s always more to build. But the foundation is solid, and we can add features incrementally because we built it right the first time.

## Closing Thoughts

The future of programming isn’t “AI replaces developers.” It’s “AI amplifies developers who know what they’re doing.”

Building Vixy proved that AI can be a powerful development partner, but only when paired with solid engineering practices. TDD, BDD, CI/CD, incremental development, and thorough documentation aren’t “old school” practices made obsolete by AI. They’re the foundation that makes AI-assisted development work.

The best engineers will be those who can architect systems, set constraints, guide execution, and verify quality. AI handles the grunt work. You handle the thinking.

If you want to experiment with this approach, the full source is on GitHub. Clone it, break it, improve it. We’re curious to see what you build.

---

*Built with Rust, tested with Cucumber, powered by Claude Code (Opus 4.5) and coffee.*

We used Claude Code, Anthropic’s CLI tool, for the entire development process. The ability to work directly in the terminal, maintain context across sessions, and integrate with our existing dev workflow (git, CI, local testing) was crucial. It felt natural, like having another engineer in the terminal with you.

**Repository:** [github.com/chainbound/vixy](https://github.com/chainbound/vixy)

**License:** MIT / Apache-2.0

## P.S.

Special shoutout to https://x.com/merklefruit for the idea, https://x.com/mert for the architecture diagram tip, and https://x.com/lwastuargo for reinforcing that guardrails are everything.

If you’re in the Ethereum ecosystem and need reliable node infrastructure, give Vixy a shot. And if you’re experimenting with AI-assisted development, remember: the guardrails matter more than the model.