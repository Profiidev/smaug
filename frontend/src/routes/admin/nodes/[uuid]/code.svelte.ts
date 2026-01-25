// @ts-ignore this will be replaced during build
const version = __version__;

export const dockerCompose = (token: string) =>
  `services:
  smaug-wings:
    image: ghcr.io/profiidev/smaug/smaug-wings:${version}
    restart: unless-stopped
    environment:
      - TOKEN=${token}
    ports:
      - "8000:8000"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock`;

export const dockerRun = (token: string) =>
  `docker run -d \\
  --name smaug-wings \\ 
  --restart unless-stopped \\
  -e TOKEN=${token} \\
  -p 8000:8000 \\
  -v /var/run/docker.sock:/var/run/docker.sock \\
  ghcr.io/profiidev/smaug/smaug-wings:${version}`;
