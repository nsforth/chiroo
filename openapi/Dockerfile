FROM docker.io/swaggerapi/swagger-ui:latest

COPY public.yaml /openapi.yaml

ENV SWAGGER_JSON=/openapi.yaml BASE_URL=/docs PERSIST_AUTHORIZATION=true CORS=true