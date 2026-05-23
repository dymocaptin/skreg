"""Unit tests for the AWS worker trigger component using Pulumi mocks."""
from __future__ import annotations

import pulumi
from pulumi.runtime import Mocks


class _TriggerMocks(Mocks):
    """Records Lambda functions and S3 bucket notifications."""

    def __init__(self) -> None:
        self.lambda_functions: dict[str, dict[str, object]] = {}
        self.bucket_notifications: dict[str, dict[str, object]] = {}

    def new_resource(
        self, args: pulumi.runtime.MockResourceArgs
    ) -> tuple[str, dict[str, object]]:
        outputs: dict[str, object] = dict(args.inputs)
        outputs["arn"] = f"arn:aws:lambda:us-west-2:123456789012:function:{args.name}"
        outputs["name"] = args.name
        if args.typ == "aws:lambda/function:Function":
            self.lambda_functions[args.name] = dict(args.inputs)
        if args.typ == "aws:s3/bucketNotification:BucketNotification":
            self.bucket_notifications[args.name] = dict(args.inputs)
        return (f"{args.name}-id", outputs)

    def call(
        self, args: pulumi.runtime.MockCallArgs
    ) -> tuple[dict[str, object], list[tuple[str, str]]]:
        return ({}, [])


pulumi.runtime.set_mocks(_TriggerMocks())

from skreg_infra.providers.aws.worker_trigger import AwsWorkerTrigger, AwsWorkerTriggerArgs  # noqa: E402


def _args() -> AwsWorkerTriggerArgs:
    return AwsWorkerTriggerArgs(
        cluster_arn="arn:aws:ecs:us-west-2:123456789012:cluster/test",
        worker_task_def_arn="arn:aws:ecs:us-west-2:123456789012:task-definition/skreg-worker:1",
        worker_task_role_arn="arn:aws:iam::123456789012:role/worker-task-role",
        exec_role_arn="arn:aws:iam::123456789012:role/exec-role",
        public_subnet_ids=["subnet-pub-1", "subnet-pub-2"],
        worker_sg_id="sg-worker123",
        bucket_name="skreg-packages-bucket",
        bucket_arn="arn:aws:s3:::skreg-packages-bucket",
    )


@pulumi.runtime.test
def test_trigger_creates_a_lambda_function() -> None:
    mocks = _TriggerMocks()
    pulumi.runtime.set_mocks(mocks)
    trigger = AwsWorkerTrigger("test-trigger", _args())

    def check(_: str) -> None:
        lambda_names = list(mocks.lambda_functions.keys())
        assert any("fn" in name for name in lambda_names), (
            f"Expected a Lambda function, found: {lambda_names}"
        )

    return trigger.outputs.lambda_arn.apply(check)


@pulumi.runtime.test
def test_trigger_creates_s3_bucket_notification() -> None:
    mocks = _TriggerMocks()
    pulumi.runtime.set_mocks(mocks)
    trigger = AwsWorkerTrigger("test-trigger2", _args())

    def check(_: str) -> None:
        assert mocks.bucket_notifications, "Expected an S3 BucketNotification resource"

    return trigger.outputs.lambda_arn.apply(check)


@pulumi.runtime.test
def test_trigger_s3_notification_filters_skill_suffix() -> None:
    mocks = _TriggerMocks()
    pulumi.runtime.set_mocks(mocks)
    trigger = AwsWorkerTrigger("test-trigger3", _args())

    def check(_: str) -> None:
        assert mocks.bucket_notifications, "Expected a BucketNotification"
        notification = next(iter(mocks.bucket_notifications.values()))
        lambda_fns: list[dict[str, object]] = notification.get("lambdaFunctions") or []
        assert lambda_fns, "Expected lambdaFunctions in BucketNotification"
        suffixes = [fn.get("filterSuffix") for fn in lambda_fns]
        assert ".skill" in suffixes, (
            f"Expected filterSuffix='.skill', got: {suffixes}"
        )

    return trigger.outputs.lambda_arn.apply(check)
