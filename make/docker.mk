.PHONY: docker-build docker-push docker-build-dev docker-build-release \
	docker-push-dev-hub docker-push-dev-ghcr docker-push-release-hub \
	docker-push-release-ghcr docker-tag-latest docker-push-latest-hub docker-push-latest-ghcr

DOCKERFILE ?= docker/Dockerfile
DOCKER_IMAGE ?= serenade-rangular-mcp
HUB_IMAGE ?= interchouette/serenade-rangular-mcp
GHCR_ORG_IMAGE ?= ghcr.io/interchouette-itc/serenade-rangular-mcp
IMAGE_TAG ?= v$(APP_VERSION)

docker-build:
	docker build -f $(DOCKERFILE) -t $(DOCKER_IMAGE) .

docker-push:
	docker push $(DOCKER_IMAGE)

docker-build-dev:
	docker build -f $(DOCKERFILE) -t $(HUB_IMAGE):dev -t $(GHCR_ORG_IMAGE):dev .

docker-build-release:
	docker build -f $(DOCKERFILE) -t $(HUB_IMAGE):$(IMAGE_TAG) -t $(GHCR_ORG_IMAGE):$(IMAGE_TAG) .

docker-push-dev-hub:
	docker push $(HUB_IMAGE):dev

docker-push-dev-ghcr:
	docker push $(GHCR_ORG_IMAGE):dev

docker-push-release-hub:
	docker push $(HUB_IMAGE):$(IMAGE_TAG)

docker-push-release-ghcr:
	docker push $(GHCR_ORG_IMAGE):$(IMAGE_TAG)

docker-tag-latest:
	docker tag $(HUB_IMAGE):$(IMAGE_TAG) $(HUB_IMAGE):latest
	docker tag $(GHCR_ORG_IMAGE):$(IMAGE_TAG) $(GHCR_ORG_IMAGE):latest

docker-push-latest-hub:
	docker push $(HUB_IMAGE):latest

docker-push-latest-ghcr:
	docker push $(GHCR_ORG_IMAGE):latest
