# FluxCode Website

Landing page for FluxCode (`index.html` and `design.md`).

## Environments

- **Production:** https://fluxcode.muaraai.com
- **Preview:** https://fluxcode-website.vercel.app

The `website` branch is used for website updates and preview deployments before merging to `main`.

## Workflow

- To update the website, check out the `website` branch (`git checkout website`) and make your changes there.
- Pushing to `website` updates the preview deployment at `fluxcode-website.vercel.app`.
- **Do not delete the `website` branch** after merging PRs to `main`. Keep it as the dedicated branch for website changes and previews.

## Local Preview

Open `website/index.html` directly in a browser, or serve the folder locally:

```bash
python3 -m http.server 8080 --directory website
```
