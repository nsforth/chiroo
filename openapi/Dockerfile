FROM docker.io/swaggerapi/swagger-ui:latest

ARG CHIROO_PUBLIC_URL=http://localhost:5173

COPY public.yaml /openapi.yaml

RUN sed -i "s#__CHIROO_PUBLIC_URL__#${CHIROO_PUBLIC_URL}#" /openapi.yaml

ENV SWAGGER_JSON=/openapi.yaml BASE_URL=/docs PERSIST_AUTHORIZATION=true CORS=true
