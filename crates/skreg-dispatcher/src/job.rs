//! K8s Job creation for skreg-worker.

use anyhow::Result;
use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::{
    Container, EnvFromSource, EnvVar, PodSpec, PodTemplateSpec, SecretEnvSource,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, ListParams, PostParams};
use kube::Client;
use log::info;

pub struct WorkerConfig<'a> {
    pub namespace: &'a str,
    pub worker_image: &'a str,
    pub pki_secret: &'a str,
    pub db_secret: &'a str,
    pub s3_bucket: &'a str,
    pub smtp_host: &'a str,
    pub smtp_port: u16,
    pub from_email: &'a str,
}

/// Returns the name of the created (or already-running) Job.
pub async fn ensure_worker_job(client: &Client, cfg: &WorkerConfig<'_>) -> Result<String> {
    let namespace = cfg.namespace;
    let jobs: Api<Job> = Api::namespaced(client.clone(), namespace);

    // Skip if any worker Job is still active.
    for job in jobs.list(&ListParams::default()).await?.items {
        if job.status.as_ref().and_then(|s| s.active).unwrap_or(0) > 0 {
            let name = job.metadata.name.clone().unwrap_or_default();
            info!("worker job {name} already active");
            return Ok(name);
        }
    }

    let job_name = format!("skreg-worker-{}", uuid::Uuid::new_v4().simple());

    let job = Job {
        metadata: ObjectMeta {
            name: Some(job_name.clone()),
            namespace: Some(namespace.to_owned()),
            labels: Some([("app".to_owned(), "skreg-worker".to_owned())].into()),
            ..Default::default()
        },
        spec: Some(JobSpec {
            backoff_limit: Some(0),
            ttl_seconds_after_finished: Some(300),
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    restart_policy: Some("Never".to_owned()),
                    containers: vec![Container {
                        name: "worker".to_owned(),
                        image: Some(cfg.worker_image.to_owned()),
                        env: Some(vec![
                            EnvVar {
                                name: "S3_BUCKET".to_owned(),
                                value: Some(cfg.s3_bucket.to_owned()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "SMTP_HOST".to_owned(),
                                value: Some(cfg.smtp_host.to_owned()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "SMTP_PORT".to_owned(),
                                value: Some(cfg.smtp_port.to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "FROM_EMAIL".to_owned(),
                                value: Some(cfg.from_email.to_owned()),
                                ..Default::default()
                            },
                        ]),
                        env_from: Some(vec![
                            EnvFromSource {
                                secret_ref: Some(SecretEnvSource {
                                    name: cfg.db_secret.to_owned(),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                            EnvFromSource {
                                secret_ref: Some(SecretEnvSource {
                                    name: cfg.pki_secret.to_owned(),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        ]),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    jobs.create(&PostParams::default(), &job).await?;
    info!("created worker job {job_name}");
    Ok(job_name)
}
